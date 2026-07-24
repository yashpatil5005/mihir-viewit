package ai.viewit.app

import android.content.Context
import android.net.Uri
import android.util.Log
import dalvik.system.DexClassLoader
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.io.FileOutputStream
import java.net.HttpURLConnection
import java.net.URL
import java.security.MessageDigest

class PluginManager(private val context: Context) {

    companion object {
        private const val TAG = "PluginManager"
        private const val CATALOG_URL = "http://127.0.0.1:8888/catalog.json"

        fun fetchManifestFromJson(obj: JSONObject): PluginManifest {
            val formats = mutableListOf<String>()
            val arr = obj.optJSONArray("supportedFormats") ?: obj.optJSONArray("formats")
            if (arr != null) {
                for (i in 0 until arr.length()) formats.add(arr.getString(i))
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
            )
        }

        fun runtimeAbi(): String {
            return when (System.getProperty("os.arch")?.lowercase()) {
                "aarch64", "arm64" -> "arm64-v8a"
                "x86_64", "amd64" -> "x86_64"
                else -> android.os.Build.SUPPORTED_ABIS.firstOrNull() ?: "arm64-v8a"
            }
        }
    }

    data class InstalledPlugin(
        val manifest: PluginManifest,
        val instance: Any,
        val mediaPlugin: ViewItPlugin?,
        val documentPlugin: ViewItDocumentPlugin?,
        val installDir: File,
    )

    private val pluginsDir = File(context.filesDir, "plugins")
    private val installed = mutableMapOf<String, InstalledPlugin>()

    fun init() {
        pluginsDir.mkdirs()
        loadInstalledPlugins()
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
        return try {
            val dir = File(pluginsDir, manifest.id)
            if (dir.exists()) {
                val existing = loadPluginFromDir(dir)
                if (existing != null) {
                    return Result.success(existing)
                }
                dir.deleteRecursively()
            }
            dir.mkdirs()

            val zipFile = File(dir, "${manifest.id}.zip")
            downloadFile(manifest.downloadUrl, zipFile, onProgress)

            if (manifest.checksum.isNotEmpty()) {
                val hash = sha256(zipFile)
                if (!hash.equals(manifest.checksum, ignoreCase = true)) {
                    zipFile.delete()
                    dir.deleteRecursively()
                    return Result.failure(Exception("Checksum mismatch"))
                }
            }

            unzip(zipFile, dir)
            zipFile.delete()

            val plugin = loadPluginFromDir(dir)
                ?: return Result.failure(Exception("Failed to load plugin"))
            val installedPlugin = plugin.copy(manifest = manifest)

            saveManifest(dir, manifest)
            installed[manifest.id] = installedPlugin
            Log.i(TAG, "Installed plugin: ${manifest.id}")
            Result.success(installedPlugin)
        } catch (e: Throwable) {
            Log.e(TAG, "Install failed: ${manifest.id}", e)
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

    fun fetchCatalog(): Result<List<PluginManifest>> {
        return try {
            val conn = URL(CATALOG_URL).openConnection() as HttpURLConnection
            conn.connectTimeout = 10_000
            conn.readTimeout = 10_000
            val body = conn.inputStream.bufferedReader().readText()
            val obj = JSONObject(body)
            val arr = obj.getJSONArray("plugins")
            val list = mutableListOf<PluginManifest>()
            for (i in 0 until arr.length()) {
                val manifest = parseManifest(arr.getJSONObject(i))
                if (manifest.abi.isEmpty() || manifest.abi == runtimeAbi()) {
                    list.add(manifest)
                }
            }
            Result.success(list)
        } catch (e: Exception) {
            Log.e(TAG, "Failed to fetch catalog", e)
            Result.failure(e)
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
                    f.name != "libffmpegkit.so" &&
                    f.name != "libffmpegkit_abidetect.so" &&
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
        }
        File(dir, "plugin.json").writeText(obj.toString(2))
    }

    private fun parseManifest(obj: JSONObject): PluginManifest {
        return fetchManifestFromJson(obj)
    }
}
