package ai.viewit.app

import android.content.Context
import android.net.Uri
import android.util.Log
import ai.viewit.app.ViewItDocumentPlugin
import dalvik.system.DexClassLoader
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.io.FileOutputStream
import java.net.HttpURLConnection
import java.net.URL
import java.security.MessageDigest
import java.security.spec.X509EncodedKeySpec
import java.security.KeyFactory
import java.util.TreeSet

class PluginManager(private val context: Context) {

    companion object {
        private const val TAG = "PluginManager"
        private const val CATALOG_URL_DEBUG = "http://127.0.0.1:8888/catalog.json"
        private const val CATALOG_URL_RELEASE = "https://viewit-plugin-catalog-temp.pages.dev/catalog.signed.json"

        private const val SUPPORTED_ABI_VERSION = 1

        // Ed25519 public key for catalog signature verification (base64 encoded)
        // Generated with: python3 scripts/sign-catalog.py generate
        // This is a DEVELOPMENT key - replace with production key before beta launch
        private const val CATALOG_PUBLIC_KEY_B64 = "hrnfmcarRcPC5tuEXGcdEIMf1a9gaXtf+DCJl0ftoTE="

        fun fetchManifestFromJson(obj: JSONObject): PluginManifest {
            val formats = mutableListOf<String>()
            val arr = obj.optJSONArray("supportedFormats") ?: obj.optJSONArray("formats")
            if (arr != null) {
                for (i in 0 until arr.length()) formats.add(arr.getString(i))
            }
            val caps = mutableListOf<String>()
            val capsArr = obj.optJSONArray("capabilities")
            if (capsArr != null) {
                for (i in 0 until capsArr.length()) caps.add(capsArr.getString(i))
            }
            return PluginManifest(
                id = obj.getString("id"),
                name = obj.getString("name"),
                version = obj.getString("version"),
                description = obj.optString("description", ""),
                minAppVersion = obj.optInt("minAppVersion", 1),
                entryClass = obj.optString("entryClass", ""),
                supportedFormats = formats,
                downloadUrl = obj.getString("downloadUrl"),
                sizeBytes = obj.optLong("sizeBytes", 0),
                installedSizeBytes = obj.optLong("installedSizeBytes", 0),
                checksum = obj.optString("checksum", ""),
                abi = obj.optString("abi", ""),
                abiVersion = obj.optInt("abiVersion", 1),
                capabilities = caps,
                runtime = obj.optString("runtime", ""),
            )
        }

        fun validateManifest(manifest: PluginManifest): List<String> {
            val errors = mutableListOf<String>()
            if (manifest.id.isBlank()) errors.add("id is required")
            if (manifest.name.isBlank()) errors.add("name is required")
            if (manifest.version.isBlank()) errors.add("version is required")
            if (manifest.entryClass.isBlank()) errors.add("entryClass is required")
            if (manifest.downloadUrl.isBlank()) errors.add("downloadUrl is required")
            if (manifest.supportedFormats.isEmpty()) errors.add("supportedFormats is required")
            if (manifest.abiVersion > SUPPORTED_ABI_VERSION) {
                errors.add("abiVersion ${manifest.abiVersion} is not supported (max $SUPPORTED_ABI_VERSION)")
            }
            return errors
        }

        fun runtimeAbi(): String {
            return android.os.Build.SUPPORTED_ABIS.firstOrNull() ?: "arm64-v8a"
        }

        fun getCatalogUrl(): String {
            val configured = BuildConfig.VIEWIT_PLUGIN_CATALOG_URL
            return if (configured.isNotBlank()) configured else if (BuildConfig.DEBUG) CATALOG_URL_DEBUG else CATALOG_URL_RELEASE
        }
    }

    data class InstalledPlugin(
        val manifest: PluginManifest,
        val instance: Any,
        val mediaPlugin: ViewItPlugin?,
        val documentPlugin: ViewItDocumentPlugin?,
        val installDir: File,
        var health: PluginHealth = PluginHealth.LOADED,
        var failCount: Int = 0,
    )

    enum class PluginHealth {
        INSTALLED,
        LOADED,
        FAILED,
        DISABLED,
        UPDATE_AVAILABLE,
    }

    private val pluginsDir = File(context.filesDir, "plugins")
    private val installed = mutableMapOf<String, InstalledPlugin>()

    fun init() {
        pluginsDir.mkdirs()
        loadInstalledPlugins()
        registerBuiltInPlugins()
    }

    private fun registerBuiltInPlugins() {
        // Register built-in native plugins (bundled in APK)
        try {
            System.loadLibrary("viewit_plugin_office_universal")
            val clazz = Class.forName("ai.viewit.plugins.officeuniversal.OfficeUniversalPlugin")
            val instance = clazz.getDeclaredConstructor().newInstance() as ViewItDocumentPlugin
            instance.initialize(context)

            val manifest = PluginManifest(
                id = "office-universal",
                name = "Office Universal",
                version = "0.1.0",
                description = "Built-in support for Office formats (docx, xlsx, pptx, odt, ods, odp, etc.)",
                minAppVersion = 1,
                entryClass = "ai.viewit.plugins.officeuniversal.OfficeUniversalPlugin",
                supportedFormats = listOf(
                    "xlsx", "xlsm", "xlsb", "xls",
                    "pptx", "pptm", "potx",
                    "odt", "ott",
                    "ods", "ots",
                    "odp", "otp",
                    "doc", "ppt"
                ),
                downloadUrl = "",
                sizeBytes = 0,
                installedSizeBytes = 0,
                checksum = "",
                abi = runtimeAbi(),
                abiVersion = 1,
                capabilities = listOf("document"),
                runtime = "native"
            )

            installed["office-universal"] = InstalledPlugin(
                manifest = manifest,
                instance = instance,
                mediaPlugin = null,
                documentPlugin = instance,
                installDir = File(context.filesDir, "plugins/office-universal")
            )
            Log.i(TAG, "Registered built-in plugin: office-universal")
        } catch (e: Throwable) {
            Log.w(TAG, "Failed to register built-in office-universal plugin", e)
        }
    }

    fun getInstalledPlugins(): List<InstalledPlugin> = installed.values.toList()

    fun canHandle(mimeType: String): InstalledPlugin? {
        return installed.values.find { it.mediaPlugin?.canHandle(mimeType) == true }
    }

    fun canHandleExt(ext: String): InstalledPlugin? {
        return installed.values.find { it.mediaPlugin?.canHandleExt(ext) == true }
    }

    fun documentPluginForExt(pluginId: String, ext: String): InstalledPlugin? {
        val plugin = installed[pluginId] ?: return null
        return if (plugin.documentPlugin?.canHandleExt(ext) == true) plugin else null
    }

    fun installPlugin(
        manifest: PluginManifest,
        onProgress: (Float) -> Unit = {},
    ): Result<InstalledPlugin> {
        val validationErrors = validateManifest(manifest)
        if (validationErrors.isNotEmpty()) {
            return Result.failure(Exception("Invalid manifest: ${validationErrors.joinToString(", ")}"))
        }

        return try {
            val dir = File(pluginsDir, manifest.id)
            if (dir.exists()) {
                val existing = installed[manifest.id]
                if (existing?.manifest?.version == manifest.version) {
                    return Result.success(existing)
                }
                val manifestFile = File(dir, "plugin.json")
                if (manifestFile.exists()) {
                    val existingManifest = parseManifest(JSONObject(manifestFile.readText()))
                    if (existingManifest.version == manifest.version) {
                        return loadPluginFromDir(dir)
                            ?.let { plugin ->
                                installed[manifest.id] = plugin
                                Result.success(plugin)
                            }
                            ?: Result.failure(Exception("Failed to load existing plugin"))
                    }
                }
                existing?.mediaPlugin?.cleanup()
                if (existing?.documentPlugin !== existing?.mediaPlugin) {
                    existing?.documentPlugin?.cleanup()
                }
                installed.remove(manifest.id)
                dir.deleteRecursively()
            }

            val stagingDir = File(pluginsDir, "${manifest.id}.staging")
            stagingDir.deleteRecursively()
            stagingDir.mkdirs()

            val zipFile = File(stagingDir, "${manifest.id}.zip")
            downloadFile(manifest.downloadUrl, zipFile, onProgress)

            if (manifest.checksum.isNotEmpty()) {
                val hash = sha256(zipFile)
                if (!hash.equals(manifest.checksum, ignoreCase = true)) {
                    stagingDir.deleteRecursively()
                    return Result.failure(Exception("Checksum mismatch"))
                }
            }

            unzip(zipFile, stagingDir)
            zipFile.delete()

            saveManifest(stagingDir, manifest)

            val plugin = loadPluginFromDir(stagingDir)
                ?: run {
                    stagingDir.deleteRecursively()
                    return Result.failure(Exception("Failed to load plugin"))
                }

            stagingDir.renameTo(dir)

            val installedPlugin = plugin.copy(manifest = manifest, health = PluginHealth.LOADED)
            installed[manifest.id] = installedPlugin
            Log.i(TAG, "Installed plugin: ${manifest.id} v${manifest.version}")
            Result.success(installedPlugin)
        } catch (e: Throwable) {
            Log.e(TAG, "Install failed: ${manifest.id}", e)
            File(pluginsDir, "${manifest.id}.staging").deleteRecursively()
            Result.failure(Exception(e.message ?: e.javaClass.simpleName, e))
        }
    }

    fun removePlugin(pluginId: String): Result<Unit> {
        return try {
            val plugin = installed.remove(pluginId)
            plugin?.mediaPlugin?.cleanup()
            if (plugin?.documentPlugin !== plugin?.mediaPlugin) {
                plugin?.documentPlugin?.cleanup()
            }
            val dir = File(pluginsDir, pluginId)
            dir.deleteRecursively()
            File(context.codeCacheDir, "plugins/$pluginId").deleteRecursively()
            Log.i(TAG, "Removed plugin: $pluginId")
            Result.success(Unit)
        } catch (e: Exception) {
            Result.failure(e)
        }
    }

    fun fetchCatalog(url: String = getCatalogUrl()): Result<List<PluginManifest>> {
        return try {
            val conn = URL(url).openConnection() as HttpURLConnection
            conn.connectTimeout = 10_000
            conn.readTimeout = 10_000
            val body = conn.inputStream.bufferedReader().readText()
            val obj = JSONObject(body)

            // Verify catalog signature if present
            if (obj.has("signature") && obj.has("catalog")) {
                val signature = obj.getString("signature")
                val catalogObj = obj.getJSONObject("catalog")
                if (!verifyCatalogSignature(canonicalJson(catalogObj), signature)) {
                    Log.e(TAG, "Catalog signature verification failed")
                    return Result.failure(Exception("Catalog signature verification failed"))
                }
                val arr = catalogObj.getJSONArray("plugins")
                val list = mutableListOf<PluginManifest>()
                for (i in 0 until arr.length()) {
                    val manifest = parseManifest(arr.getJSONObject(i))
                    if (isCompatible(manifest) && isInstallable(manifest)) {
                        list.add(manifest)
                    }
                }
                Result.success(list)
            } else {
                // Fallback for unsigned catalog (debug/dev)
                Log.w(TAG, "Catalog is not signed - accepting for debug/dev")
                val arr = obj.getJSONArray("plugins")
                val list = mutableListOf<PluginManifest>()
                for (i in 0 until arr.length()) {
                    val manifest = parseManifest(arr.getJSONObject(i))
                    if (isCompatible(manifest) && isInstallable(manifest)) {
                        list.add(manifest)
                    }
                }
                Result.success(list)
            }
        } catch (e: Exception) {
            Log.e(TAG, "Failed to fetch catalog", e)
            Result.failure(e)
        }
    }

    private fun verifyCatalogSignature(catalogJson: String, signatureB64: String): Boolean {
        return try {
            val publicKeyBytes = android.util.Base64.decode(CATALOG_PUBLIC_KEY_B64, android.util.Base64.DEFAULT)
            val signatureBytes = android.util.Base64.decode(signatureB64, android.util.Base64.DEFAULT)

            val keySpec = X509EncodedKeySpec(ed25519SubjectPublicKeyInfo(publicKeyBytes))
            val keyFactory = KeyFactory.getInstance("Ed25519")
            val publicKey = keyFactory.generatePublic(keySpec)

            val sig = java.security.Signature.getInstance("Ed25519")
            sig.initVerify(publicKey)
            sig.update(catalogJson.toByteArray(Charsets.UTF_8))
            sig.verify(signatureBytes)
        } catch (e: Exception) {
            Log.e(TAG, "Signature verification failed", e)
            false
        }
    }

    private fun isCompatible(manifest: PluginManifest): Boolean {
        val abiMatches = manifest.abi.isEmpty() || manifest.abi == runtimeAbi()
        return abiMatches && manifest.minAppVersion <= BuildConfig.VERSION_CODE
    }

    private fun isInstallable(manifest: PluginManifest): Boolean {
        return manifest.downloadUrl.isNotBlank() && manifest.checksum.isNotBlank() && manifest.sizeBytes > 0
    }

    private fun ed25519SubjectPublicKeyInfo(rawKey: ByteArray): ByteArray {
        if (rawKey.size != 32) return rawKey
        val prefix = byteArrayOf(
            0x30, 0x2a,
            0x30, 0x05,
            0x06, 0x03, 0x2b, 0x65, 0x70,
            0x03, 0x21, 0x00,
        )
        return prefix + rawKey
    }

    private fun canonicalJson(value: Any?): String {
        return when (value) {
            null, JSONObject.NULL -> "null"
            is JSONObject -> {
                val keys = TreeSet<String>()
                value.keys().forEachRemaining { keys.add(it) }
                keys.joinToString(prefix = "{", postfix = "}", separator = ",") { key ->
                    "${JSONObject.quote(key)}:${canonicalJson(value.get(key))}"
                }
            }
            is JSONArray -> {
                (0 until value.length()).joinToString(prefix = "[", postfix = "]", separator = ",") { i ->
                    canonicalJson(value.get(i))
                }
            }
            is String -> JSONObject.quote(value).replace("\\/", "/")
            is Number, is Boolean -> value.toString()
            else -> JSONObject.quote(value.toString())
        }
    }

    private fun loadInstalledPlugins() {
        val dirs = pluginsDir.listFiles() ?: return
        for (dir in dirs) {
            if (!dir.isDirectory) continue
            val plugin = loadPluginFromDir(dir)
            if (plugin != null) {
                installed[plugin.manifest.id] = plugin
                Log.i(TAG, "Loaded plugin: ${plugin.manifest.id}")
            }
        }
    }

    private fun loadPluginFromDir(dir: File): InstalledPlugin? {
        return try {
            val manifestFile = File(dir, "plugin.json")
            if (!manifestFile.exists()) return null
            val manifest = parseManifest(JSONObject(manifestFile.readText()))

            // Native libs may live under lib/<abi>/ or directly under <abi>/
            val nativeDir = File(dir, "lib")
            if (nativeDir.exists()) {
                loadNativeLibs(nativeDir)
            } else {
                loadNativeLibs(dir)
            }

            val dexFile = File(dir, "classes.dex")
            if (!dexFile.exists()) {
                val dexDir = File(dir, "dex")
                if (dexDir.exists()) {
                    val dex = dexDir.listFiles()?.firstOrNull { it.extension == "dex" }
                    if (dex != null) {
                        loadDex(manifest, dex, dir)
                    } else null
                } else null
            } else {
                loadDex(manifest, dexFile, dir)
            }
        } catch (e: Throwable) {
            Log.e(TAG, "Failed to load plugin from ${dir.name}", e)
            null
        }
    }

    private fun loadDex(manifest: PluginManifest, dexFile: File, pluginDir: File): InstalledPlugin? {
        // Android 8+ forbids loading writable DEX files — copy to codeCacheDir and lock RO
        val cacheDexDir = File(context.codeCacheDir, "plugins/${manifest.id}")
        cacheDexDir.mkdirs()
        val lockedDex = File(cacheDexDir, "classes.dex")
        if (!lockedDex.exists() || lockedDex.length() != dexFile.length()) {
            dexFile.copyTo(lockedDex, overwrite = true)
        }
        lockedDex.setReadOnly()

        val optimizedDir = File(cacheDexDir, "opt")
        optimizedDir.mkdirs()

        // Prefer device ABI native lib dir as library search path
        val abi = runtimeAbi()
        val libPath = listOf(
            File(pluginDir, "lib/$abi"),
            File(pluginDir, abi),
            File(pluginDir, "lib"),
        ).firstOrNull { it.isDirectory }?.absolutePath

        val classLoader = DexClassLoader(
            lockedDex.absolutePath,
            optimizedDir.absolutePath,
            libPath,
            context.classLoader,
        )
        val clazz = classLoader.loadClass(manifest.entryClass)
        val instance = clazz.getDeclaredConstructor().newInstance()
        val mediaPlugin = instance as? ViewItPlugin
        val documentPlugin = instance as? ViewItDocumentPlugin
        if (mediaPlugin == null && documentPlugin == null) {
            throw IllegalArgumentException("Plugin ${manifest.id} implements no supported ViewIt ABI")
        }
        mediaPlugin?.initialize(context)
        if (documentPlugin !== mediaPlugin) {
            documentPlugin?.initialize(context)
        }

        return InstalledPlugin(
            manifest = manifest,
            instance = instance,
            mediaPlugin = mediaPlugin,
            documentPlugin = documentPlugin,
            installDir = pluginDir,
        )
    }

    private fun loadNativeLibs(libDir: File) {
        val abi = runtimeAbi()
        val candidates = listOf(
            File(libDir, abi),
            File(libDir, "lib/$abi"),
            libDir,
        )
        for (archDir in candidates) {
            if (!archDir.isDirectory) continue
            val soFiles = archDir.listFiles { f ->
                f.isFile && f.extension == "so" &&
                    !f.name.startsWith("libviewit_plugin_")
            }?.toMutableList() ?: continue
            // Retry loop: FFmpeg libs have a dependency chain (avcodec→swresample→avutil etc).
            // Keep retrying failed libs until all loaded or no progress is made.
            var lastError: UnsatisfiedLinkError? = null
            var progress = true
            while (soFiles.isNotEmpty() && progress) {
                progress = false
                val it = soFiles.iterator()
                while (it.hasNext()) {
                    val so = it.next()
                    try {
                        System.load(so.absolutePath)
                        Log.i(TAG, "Loaded native lib: ${so.name}")
                        it.remove()
                        progress = true
                        lastError = null
                    } catch (e: UnsatisfiedLinkError) {
                        lastError = e
                    }
                }
            }
            for (so in soFiles) {
                Log.w(TAG, "Failed to load ${so.name}: ${lastError?.message}")
            }
            break
        }
    }

    private fun downloadFile(urlStr: String, dest: File, onProgress: (Float) -> Unit) {
        val conn = URL(urlStr).openConnection() as HttpURLConnection
        conn.connectTimeout = 30_000
        conn.readTimeout = 60_000
        conn.connect()

        val total = conn.contentLength.toLong()
        var downloaded = 0L

        conn.inputStream.use { input ->
            FileOutputStream(dest).use { output ->
                val buf = ByteArray(8192)
                var read: Int
                while (input.read(buf).also { read = it } != -1) {
                    output.write(buf, 0, read)
                    downloaded += read
                    if (total > 0) {
                        onProgress(downloaded.toFloat() / total)
                    }
                }
            }
        }
    }

    private fun unzip(zipFile: File, destDir: File) {
        val zip = java.util.zip.ZipFile(zipFile)
        zip.entries().asSequence().forEach { entry ->
            val outFile = File(destDir, entry.name)
            if (entry.isDirectory) {
                outFile.mkdirs()
            } else {
                outFile.parentFile?.mkdirs()
                zip.getInputStream(entry).use { input ->
                    FileOutputStream(outFile).use { output ->
                        input.copyTo(output)
                    }
                }
            }
        }
        zip.close()
    }

    private fun sha256(file: File): String {
        val digest = MessageDigest.getInstance("SHA-256")
        file.inputStream().use { input ->
            val buf = ByteArray(8192)
            var read: Int
            while (input.read(buf).also { read = it } != -1) {
                digest.update(buf, 0, read)
            }
        }
        return digest.digest().joinToString("") { "%02x".format(it) }
    }

    private fun saveManifest(dir: File, manifest: PluginManifest) {
        val obj = JSONObject().apply {
            put("id", manifest.id)
            put("name", manifest.name)
            put("version", manifest.version)
            put("description", manifest.description)
            put("minAppVersion", manifest.minAppVersion)
            put("entryClass", manifest.entryClass)
            put("supportedFormats", JSONArray(manifest.supportedFormats))
            put("downloadUrl", manifest.downloadUrl)
            put("sizeBytes", manifest.sizeBytes)
            put("installedSizeBytes", manifest.installedSizeBytes)
            put("checksum", manifest.checksum)
            put("abi", manifest.abi)
            put("abiVersion", manifest.abiVersion)
            if (manifest.capabilities.isNotEmpty()) put("capabilities", JSONArray(manifest.capabilities))
            if (manifest.runtime.isNotEmpty()) put("runtime", manifest.runtime)
        }
        File(dir, "plugin.json").writeText(obj.toString(2))
    }

    private fun parseManifest(obj: JSONObject): PluginManifest {
        return fetchManifestFromJson(obj)
    }
}
