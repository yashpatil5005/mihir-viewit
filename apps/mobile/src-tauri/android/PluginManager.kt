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
import java.util.TreeSet
import net.i2p.crypto.eddsa.EdDSAEngine
import net.i2p.crypto.eddsa.EdDSAPublicKey
import net.i2p.crypto.eddsa.spec.EdDSANamedCurveTable
import net.i2p.crypto.eddsa.spec.EdDSAPublicKeySpec

class PluginManager(private val context: Context) {

    companion object {
        private const val TAG = "PluginManager"
        private const val CATALOG_URL_DEBUG = "http://127.0.0.1:8888/catalog.json"
        private const val CATALOG_URL_RELEASE = "https://omnia.mihirpatil.co/catalog.signed.json"

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
                base = obj.optString("base", "view"),
                runtime = obj.optString("runtime", ""),
                jsEntry = obj.optString("jsEntry", "web/index.js"),
                cssEntry = obj.optString("cssEntry", ""),
            )
        }

        fun validateManifest(manifest: PluginManifest): List<String> {
            return PluginRuntimePolicy.validateManifest(manifest)
        }

        fun runtimeAbi(): String {
            return PluginRuntimePolicy.selectRuntimeAbi(android.os.Build.SUPPORTED_ABIS)
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
    // Android cannot dlclose a loaded native library, so a native plugin that was
    // loaded this process stays loaded even after removePlugin. Cache the loaded
    // instance (version+checksum) so a reinstall of the SAME artifact reuses it
    // instead of re-attempting System.load on an already-open path (which throws
    // "already opened by ClassLoader X; can't open in ClassLoader Y").
    private val warmLoaded = mutableMapOf<String, InstalledPlugin>()

    fun init() {
        pluginsDir.mkdirs()
        loadInstalledPlugins()
        registerBuiltInPlugins()
    }

    private fun registerBuiltInPlugins() {
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

    /** Look up a plugin by id without an extension filter (archive bridge). */
    fun documentPluginForId(pluginId: String): InstalledPlugin? {
        return installed[pluginId]
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

            // Download to a temp OUTSIDE the staging dir: installZipPayload wipes
            // its own staging dir, which previously deleted the just-downloaded
            // zip and made the subsequent unzip throw FileNotFoundException/ENOENT.
            val zipFile = java.io.File.createTempFile("pkg-${manifest.id}-", ".zip", context.cacheDir)
            try {
                downloadFile(manifest.downloadUrl, zipFile, onProgress)
                installZipPayload(zipFile, manifest, onProgress)
            } finally {
                zipFile.delete()
            }
        } catch (e: Throwable) {
            Log.e(TAG, "Install failed: ${manifest.id}", e)
            File(pluginsDir, "${manifest.id}.staging").deleteRecursively()
            Result.failure(Exception(e.message ?: e.javaClass.simpleName, e))
        }
    }

    /** Debug/test helper — install a plugin from a ZIP already on the device (no download). */
    fun installLocalZip(zipPath: String, onProgress: (Float) -> Unit = {}): Result<InstalledPlugin> {
        return try {
            val zipFile = File(zipPath)
            if (!zipFile.exists()) return Result.failure(Exception("Local plugin zip not found: $zipPath"))
            val entry = java.util.zip.ZipFile(zipFile).use { z ->
                z.entries().asSequence().firstOrNull { it.name == "plugin.json" }
                ?: return@use null
            } ?: return Result.failure(Exception("Missing plugin.json in zip"))
            val manifestJson = java.util.zip.ZipFile(zipFile).use { z ->
                z.getInputStream(entry).readBytes().toString(Charsets.UTF_8)
            }
            val manifest = fetchManifestFromJson(JSONObject(manifestJson))
            val validationErrors = validateManifest(manifest)
            if (validationErrors.isNotEmpty()) {
                return Result.failure(Exception("Invalid manifest: ${validationErrors.joinToString(", ")}"))
            }
            onProgress(0.3f)
            installZipPayload(zipFile, manifest, onProgress)
        } catch (e: Throwable) {
            Log.e(TAG, "Local install failed: $zipPath", e)
            Result.failure(Exception(e.message ?: e.javaClass.simpleName, e))
        }
    }

    private fun installZipPayload(
        zipFile: File,
        manifest: PluginManifest,
        onProgress: (Float) -> Unit,
    ): Result<InstalledPlugin> {
        return try {
            if (manifest.checksum.isNotEmpty()) {
                val hash = sha256(zipFile)
                if (!hash.equals(manifest.checksum, ignoreCase = true)) {
                    return Result.failure(Exception("Checksum mismatch"))
                }
            }

            // A native plugin loaded this process stays loaded (Android cannot
            // dlclose it). Reinstall of the SAME artifact reuses the loaded
            // instance; an UPGRADE persists the new files and asks for a restart
            // (reloading the .so into a fresh classloader would throw an
            // UnsatisfiedLinkError cross-loader).
            val warm = warmLoaded[manifest.id]
            when (PluginRuntimePolicy.warmInstallAction(
                warm?.manifest,
                warm?.let { it.mediaPlugin != null || it.documentPlugin != null } == true,
                manifest,
            )) {
                PluginRuntimePolicy.WarmInstallAction.REUSE -> {
                    checkNotNull(warm)
                    val stagingDir = File(pluginsDir, "${manifest.id}.staging")
                    stagingDir.deleteRecursively()
                    stagingDir.mkdirs()
                    unzip(zipFile, stagingDir, manifest.installedSizeBytes)
                    val dir2 = File(pluginsDir, manifest.id)
                    dir2.deleteRecursively()
                    stagingDir.renameTo(dir2)
                    val reinstalled = warm.copy(manifest = manifest, installDir = dir2)
                    installed[manifest.id] = reinstalled
                    onProgress(1f)
                    Log.i(TAG, "Reused warm-loaded plugin (native stays loaded): ${manifest.id} v${manifest.version}")
                    return Result.success(reinstalled)
                }
                PluginRuntimePolicy.WarmInstallAction.STAGE_FOR_RESTART -> {
                    checkNotNull(warm)
                    val stagingDir = File(pluginsDir, "${manifest.id}.staging")
                    stagingDir.deleteRecursively()
                    stagingDir.mkdirs()
                    unzip(zipFile, stagingDir, manifest.installedSizeBytes)
                    saveManifest(stagingDir, manifest)
                    val dir2 = File(pluginsDir, manifest.id)
                    dir2.deleteRecursively()
                    stagingDir.renameTo(dir2)
                    installed[manifest.id] = warm // keep this session working on the old loaded instance
                    Log.w(TAG, "Native plugin update staged; restart to apply: ${manifest.id} → v${manifest.version}")
                    return Result.failure(Exception("Update downloaded. Restart the app to apply it."))
                }
                PluginRuntimePolicy.WarmInstallAction.LOAD -> Unit
            }

            val dir = File(pluginsDir, manifest.id)
            if (dir.exists()) {
                val existing = installed[manifest.id]
                if (existing?.manifest?.version == manifest.version) {
                    return Result.success(existing)
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

            onProgress(0.6f)
            unzip(zipFile, stagingDir, manifest.installedSizeBytes)
            saveManifest(stagingDir, manifest)

            val plugin = loadPluginFromDir(stagingDir)
                ?: run {
                    stagingDir.deleteRecursively()
                    return Result.failure(Exception("Failed to load plugin"))
                }

            // loadPluginFromDir records the staging path as installDir; point it at
            // the final dir after the (possibly successful) rename so asset reads
            // (loadPluginBundle/pluginAssetB64) work in the same process.
            val moved = stagingDir.renameTo(dir)
            if (!moved) Log.w(TAG, "Plugin dir rename failed; keeping staging path: ${manifest.id}")
            val finalDir = if (moved) dir else stagingDir

            val installedPlugin = plugin.copy(manifest = manifest, health = PluginHealth.LOADED, installDir = finalDir)
            installed[manifest.id] = installedPlugin
            if (plugin.mediaPlugin != null || plugin.documentPlugin != null) {
                // Native plugins are the warm-reuse case; JS plugins cheap to reload.
                warmLoaded[manifest.id] = installedPlugin
            }
            onProgress(1f)
            Log.i(TAG, "Installed plugin: ${manifest.id} v${manifest.version}")
            Result.success(installedPlugin)
        } catch (e: Throwable) {
            Log.e(TAG, "Install failed: ${manifest.id}", e)
            File(pluginsDir, "${manifest.id}.staging").deleteRecursively()
            Result.failure(Exception(e.message ?: e.javaClass.simpleName, e))
        }
    }

    /** Base64 of a plugin asset file (used by runtime=js plugins to load bundles in the WebView). */
    fun pluginAssetB64(pluginId: String, relPath: String): String? {
        val plugin = installed[pluginId] ?: return null
        return try {
            val file = PluginRuntimePolicy.zipEntryDestination(plugin.installDir, relPath)
            if (!file.isFile || file.length() > 16L * 1024 * 1024) return null
            java.util.Base64.getEncoder().encodeToString(file.readBytes())
        } catch (e: Exception) {
            Log.e(TAG, "Read plugin asset failed: $pluginId/$relPath", e)
            null
        }
    }

    fun removePlugin(pluginId: String): Result<Unit> {
        if (!PluginRuntimePolicy.isValidPluginId(pluginId)) {
            return Result.failure(IllegalArgumentException("Invalid plugin id"))
        }
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
                if (!PluginRuntimePolicy.acceptsUnsignedCatalog(BuildConfig.DEBUG)) {
                    return Result.failure(Exception("Plugin catalog signature is required"))
                }
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

            val keySpec = EdDSAPublicKeySpec(publicKeyBytes, EdDSANamedCurveTable.getByName("Ed25519"))
            val publicKey = EdDSAPublicKey(keySpec)
            val sig = EdDSAEngine(MessageDigest.getInstance("SHA-512"))
            sig.initVerify(publicKey)
            sig.update(catalogJson.toByteArray(Charsets.UTF_8))
            sig.verify(signatureBytes)
        } catch (e: Exception) {
            Log.e(TAG, "Signature verification failed", e)
            false
        }
    }

    private fun isCompatible(manifest: PluginManifest): Boolean {
        return PluginRuntimePolicy.isCompatible(manifest, runtimeAbi(), BuildConfig.VERSION_CODE)
    }

    private fun isInstallable(manifest: PluginManifest): Boolean {
        return PluginRuntimePolicy.isInstallable(manifest)
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

            if (manifest.runtime == "js") {
                val entry = PluginRuntimePolicy.zipEntryDestination(dir, manifest.jsEntry)
                if (!entry.isFile) {
                    Log.e(TAG, "JS plugin ${manifest.id} missing entry ${manifest.jsEntry}")
                    return null
                }
                return InstalledPlugin(
                    manifest = manifest,
                    instance = Unit,
                    mediaPlugin = null,
                    documentPlugin = null,
                    installDir = dir,
                )
            }

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

    private fun unzip(zipFile: File, destDir: File, declaredInstalledSize: Long) {
        val maxBytes = PluginRuntimePolicy.installedSizeLimit(declaredInstalledSize)
        var totalBytes = 0L
        var entryCount = 0
        java.util.zip.ZipFile(zipFile).use { zip ->
            zip.entries().asSequence().forEach { entry ->
                entryCount++
                PluginRuntimePolicy.requireExtractionWithinLimits(entryCount, totalBytes, maxBytes)
                val outFile = PluginRuntimePolicy.zipEntryDestination(destDir, entry.name)
                if (entry.isDirectory) {
                    outFile.mkdirs()
                } else {
                    outFile.parentFile?.mkdirs()
                    zip.getInputStream(entry).use { input ->
                        FileOutputStream(outFile).use { output ->
                            val buffer = ByteArray(8192)
                            while (true) {
                                val read = input.read(buffer)
                                if (read < 0) break
                                totalBytes += read
                                PluginRuntimePolicy.requireExtractionWithinLimits(entryCount, totalBytes, maxBytes)
                                output.write(buffer, 0, read)
                            }
                        }
                    }
                }
            }
        }
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
            if (manifest.base.isNotEmpty() && manifest.base != "view") put("base", manifest.base)
            if (manifest.runtime.isNotEmpty()) put("runtime", manifest.runtime)
            if (manifest.runtime == "js") put("jsEntry", manifest.jsEntry)
            if (manifest.runtime == "js" && manifest.cssEntry.isNotEmpty()) put("cssEntry", manifest.cssEntry)
        }
        File(dir, "plugin.json").writeText(obj.toString(2))
    }

    private fun parseManifest(obj: JSONObject): PluginManifest {
        return fetchManifestFromJson(obj)
    }
}
