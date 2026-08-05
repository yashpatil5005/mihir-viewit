package ai.viewit.app

import android.content.Intent
import android.database.Cursor
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.provider.MediaStore
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.io.FileOutputStream
import java.security.MessageDigest

class MainActivity : TauriActivity() {
  private var bridgeWebView: WebView? = null

  /** Archive "Save entry as…" flow: SAF ACTION_CREATE_DOCUMENT handed to the
   *  system picker; bytes are written on onActivityResult. */
  private data class PendingSave(
    val plugin: ArchivePlugin,
    val uri: String,
    val name: String,
    val entryName: String,
    val displayName: String,
    val callbackId: String,
  )
  private val pendingSaves = java.util.concurrent.ConcurrentHashMap<Int, PendingSave>()
  private var saveRequestCode = 9001

  companion object {
    const val ACTION_DEBUG_OPEN = "ai.viewit.app.action.DEBUG_OPEN"
    const val ACTION_DEBUG_INSTALL_PLUGIN = "ai.viewit.app.action.DEBUG_INSTALL_PLUGIN"
  }

  override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
    super.onActivityResult(requestCode, resultCode, data)
    val pending = pendingSaves.remove(requestCode) ?: return
    val webView = bridgeWebView ?: return
    val payload = JSONObject().apply {
      put("id", pending.callbackId)
      put("op", "save")
    }
    if (resultCode != RESULT_OK || data?.data == null) {
      payload.put("ok", false)
      payload.put("error", "Save cancelled")
      webView.post { webView.evaluateJavascript("window._pluginArchiveCallback && window._pluginArchiveCallback($payload)", null) }
      return
    }
    val outUri = data.data!!
    Thread {
      try {
        val tmp = File(applicationContext.cacheDir, "save_stage_$requestCode")
        val thread = Thread.currentThread()
        val prev = thread.contextClassLoader
        val resultJson = try {
          thread.contextClassLoader = pending.plugin.javaClass.classLoader
          pending.plugin.extractEntryToFile(Uri.parse(pending.uri), pending.name, pending.entryName, tmp)
        } finally {
          thread.contextClassLoader = prev
        }
        val result = JSONObject(resultJson)
        if (!result.optBoolean("ok", false)) {
          payload.put("ok", false)
          payload.put("error", result.optString("error", "Extraction failed"))
        } else {
          contentResolver.openOutputStream(outUri)?.use { it.write(tmp.readBytes()) } ?: error("No writable stream")
          payload.put("ok", true)
          payload.put("size", tmp.length())
        }
        tmp.delete()
      } catch (e: Throwable) {
        payload.put("ok", false)
        payload.put("error", e.message ?: e.javaClass.simpleName)
      }
      webView.post { webView.evaluateJavascript("window._pluginArchiveCallback && window._pluginArchiveCallback($payload)", null) }
    }.start()
  }

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    WebView.setWebContentsDebuggingEnabled(true)
    super.onCreate(savedInstanceState)
    consumeIncomingIntent(intent)
  }

  override fun onWebViewCreate(webView: WebView) {
    super.onWebViewCreate(webView)
    bridgeWebView = webView
    webView.addJavascriptInterface(AndroidBridge(webView), "AndroidBridge")
  }

  override fun onNewIntent(intent: Intent) {
    super.onNewIntent(intent)
    setIntent(intent)
    consumeIncomingIntent(intent)
  }

  private fun consumeIncomingIntent(intent: Intent?) {
    if (intent == null) return
    val action = intent.action ?: return
    val isDebugOpen = action == ACTION_DEBUG_OPEN
    val isDebugInstall = action == ACTION_DEBUG_INSTALL_PLUGIN
    if (
      !isDebugOpen &&
      !isDebugInstall &&
      action != Intent.ACTION_VIEW &&
      action != Intent.ACTION_SEND &&
      action != Intent.ACTION_SEND_MULTIPLE
    ) {
      return
    }

    val lines = mutableListOf<String>()

    @Suppress("DEPRECATION")
    val stream = intent.getParcelableExtra<Uri>(Intent.EXTRA_STREAM)
    if (stream != null) {
      lines.add(grantWithDisplayName(stream, intent.type))
    }

    if (action == Intent.ACTION_SEND_MULTIPLE) {
      @Suppress("DEPRECATION")
      val list = intent.getParcelableArrayListExtra<Uri>(Intent.EXTRA_STREAM)
      list?.forEach { lines.add(grantWithDisplayName(it, intent.type)) }
    }

    intent.data?.let { lines.add(grantWithDisplayName(it, intent.type)) }

    // Debug-driven open: `am start -a ${ACTION_DEBUG_OPEN} --es uri ... --es name ...`
    // carries an explicit name (and optional ext/plugin) so extension-less SAF URIs
    // from scripts / automation resolve exactly like filesystem paths.
    if (isDebugOpen) {
      val debugUri = intent.getStringExtra("uri")
      val debugName = intent.getStringExtra("name")
      val debugExt = intent.getStringExtra("ext")
      val debugPlugin = intent.getStringExtra("plugin")
      if (debugUri != null && intent.data == null) {
        lines.add(debugRecord(debugUri, debugName, debugExt, debugPlugin, intent.type))
      }
    }

    if (isDebugInstall) {
      // `am start -a ${ACTION_DEBUG_INSTALL_PLUGIN} --es path <abs zip>` — local,
      // download-free install for verifying runtime=js plugins on-device.
      val zipPath = intent.getStringExtra("path") ?: return
      val pm = (application as? ViewItApp)?.pluginManager ?: return
      Thread {
        try {
          val result = pm.installLocalZip(zipPath)
          runOnUiThread {
            android.util.Log.i("ViewIt", "DEBUG_INSTALL_PLUGIN ${zipPath} -> ${result.isSuccess} (${result.exceptionOrNull()?.message})")
            bridgeWebView?.evaluateJavascript(
              "window.__viewitDebugInstall && window.__viewitDebugInstall(${result.isSuccess})",
              null
            )
          }
        } catch (e: Exception) {
          android.util.Log.e("ViewIt", "DEBUG_INSTALL_PLUGIN failed", e)
        }
      }.start()
      return
    }

    if (lines.isEmpty()) return

    try {
      val f = File(applicationContext.filesDir, "viewit_pending_opens.txt")
      f.writeText(lines.joinToString("\n") + "\n")
      notifyWebViewOpened(lines)
    } catch (e: Exception) {
      android.util.Log.e("ViewIt", "pending opens write failed", e)
    }
  }

  private fun debugRecord(uri: String, name: String?, ext: String?, plugin: String?, mime: String?): String {
    val displayName = name ?: Uri.parse(uri)?.lastPathSegment ?: ""
    return "$uri\t$displayName\t${mime ?: ""}\t${ext ?: ""}\t${plugin ?: ""}"
  }

  /** Notify an already-loaded WebView of new opens (native-side, in-process). */
  private fun notifyWebViewOpened(lines: List<String>) {
    val webView = bridgeWebView ?: return
    if (lines.isEmpty()) return
    val payload = JSONArray()
    lines.forEach { line ->
      val parts = line.split("\t", limit = 5)
      payload.put(JSONObject().apply {
        put("uri", parts.getOrNull(0)?.trim() ?: "")
        put("name", parts.getOrNull(1)?.trim() ?: "")
        put("mime", parts.getOrNull(2)?.trim() ?: "")
        put("ext", parts.getOrNull(3)?.trim() ?: "")
        put("plugin", parts.getOrNull(4)?.trim() ?: "")
      })
    }
    webView.post {
      webView.evaluateJavascript("window.__viewitAndroidOpened && window.__viewitAndroidOpened(${payload.toString()})", null)
    }
  }

  private fun drainPendingOpenUris(): JSONArray {
    val result = JSONArray()
    val f = File(applicationContext.filesDir, "viewit_pending_opens.txt")
    if (!f.exists()) return result
    try {
      f.readLines().forEach { line ->
        val parts = line.split("\t", limit = 5)
        val uri = parts.getOrNull(0)?.trim()
        if (uri.isNullOrEmpty()) return@forEach
        result.put(JSONObject().apply {
          put("uri", uri)
          put("name", parts.getOrNull(1)?.trim() ?: "")
          put("mime", parts.getOrNull(2)?.trim() ?: "")
          put("ext", parts.getOrNull(3)?.trim() ?: "")
          put("plugin", parts.getOrNull(4)?.trim() ?: "")
        })
      }
    } finally {
      f.delete()
    }
    return result
  }

  private fun grantWithDisplayName(uri: Uri, mimeType: String?): String {
    try {
      val flags = Intent.FLAG_GRANT_READ_URI_PERMISSION
      if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.KITKAT) {
        try {
          contentResolver.takePersistableUriPermission(uri, flags)
        } catch (_: SecurityException) {
        }
      }
    } catch (_: Exception) {
    }
    val displayName = queryDisplayName(uri)
    return if (displayName != null) {
      "${uri}\t$displayName\t${mimeType ?: ""}"
    } else {
      "${uri}\t\t${mimeType ?: ""}"
    }
  }

  private fun queryDisplayName(uri: Uri): String? {
    if (uri.scheme == "file") return uri.lastPathSegment
    val projection = arrayOf(MediaStore.MediaColumns.DISPLAY_NAME)
    return try {
      val cursor: Cursor? = contentResolver.query(uri, projection, null, null, null)
      cursor?.use {
        if (it.moveToFirst()) {
          val idx = it.getColumnIndex(MediaStore.MediaColumns.DISPLAY_NAME)
          if (idx >= 0) it.getString(idx) else null
        } else null
      }
    } catch (_: Exception) {
      uri.lastPathSegment
    }
  }

  inner class AndroidBridge(private val webView: WebView) {
    @JavascriptInterface
    fun drainPendingOpenUris(): String = this@MainActivity.drainPendingOpenUris().toString()

    @JavascriptInterface
    fun getMimeType(uri: String): String {
      return try {
        contentResolver.getType(Uri.parse(uri)) ?: ""
      } catch (_: Exception) {
        ""
      }
    }

    @JavascriptInterface
    fun getDisplayName(uri: String): String {
      return queryDisplayName(Uri.parse(uri)) ?: Uri.parse(uri).lastPathSegment ?: ""
    }

    @JavascriptInterface
    fun launchVideoPlayer(uri: String, title: String, ext: String) {
      runOnUiThread {
        val intent = Intent(this@MainActivity, VideoPlayerActivity::class.java).apply {
          putExtra(VideoPlayerActivity.EXTRA_URI, uri)
          putExtra(VideoPlayerActivity.EXTRA_TITLE, title)
          putExtra(VideoPlayerActivity.EXTRA_EXT, ext)
          addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        }
        startActivity(intent)
      }
    }

    @JavascriptInterface
    fun materializeExternalUri(uri: String, ext: String): String {
      val parsed = Uri.parse(uri)
      if (parsed.scheme == "file") {
        val path = parsed.path ?: throw IllegalArgumentException("Invalid file URI")
        val appDir = applicationContext.cacheDir.canonicalFile
        val source = File(path).canonicalFile
        if (source.path == appDir.path || source.path.startsWith(appDir.path + File.separator)) {
          return parsed.toString()
        }
      }

      val safeExt = ext.lowercase().replace(Regex("[^a-z0-9]"), "").ifEmpty { "bin" }
      val digest = MessageDigest.getInstance("SHA-256")
        .digest(uri.toByteArray(Charsets.UTF_8))
        .take(12)
        .joinToString("") { "%02x".format(it) }
      val dest = File(applicationContext.cacheDir, "external-$digest.$safeExt")
      if (dest.exists()) dest.delete()

      val input = if (parsed.scheme == "content") {
        contentResolver.openInputStream(parsed)
      } else {
        File(parsed.path ?: throw IllegalArgumentException("Invalid file URI")).inputStream()
      } ?: throw IllegalArgumentException("Unable to open URI")

      input.use { source ->
        FileOutputStream(dest).use { output ->
          val buffer = ByteArray(DEFAULT_BUFFER_SIZE)
          while (true) {
            val read = source.read(buffer)
            if (read <= 0) break
            output.write(buffer, 0, read)
          }
        }
      }
      return Uri.fromFile(dest).toString()
    }

    @JavascriptInterface
    fun listPlugins(): String {
      val pm = (application as? ViewItApp)?.pluginManager ?: return "[]"
      val arr = JSONArray()
      for (p in pm.getInstalledPlugins()) {
        arr.put(JSONObject().apply {
          put("id", p.manifest.id)
          put("name", p.manifest.name)
          put("version", p.manifest.version)
          put("description", p.manifest.description)
          put("entryClass", p.manifest.entryClass)
          put("formats", JSONArray(p.manifest.supportedFormats))
          put("sizeBytes", p.manifest.sizeBytes)
          put("installedSizeBytes", p.manifest.installedSizeBytes)
          put("checksum", p.manifest.checksum)
          put("abi", p.manifest.abi)
          put("runtime", p.manifest.runtime.ifBlank { "native" })
          if (p.manifest.runtime == "js") put("jsEntry", p.manifest.jsEntry)
          if (p.manifest.runtime == "js" && p.manifest.cssEntry.isNotEmpty()) put("cssEntry", p.manifest.cssEntry)
        })
      }
      return arr.toString()
    }

    @JavascriptInterface
    fun pluginAssetB64(pluginId: String, relPath: String): String {
      val pm = (application as? ViewItApp)?.pluginManager ?: return ""
      return pm.pluginAssetB64(pluginId, relPath) ?: ""
    }

    @JavascriptInterface
    fun loadPluginBundle(pluginId: String): String {
      val pm = (application as? ViewItApp)?.pluginManager ?: return ""
      val plugin = pm.getInstalledPlugins().firstOrNull { it.manifest.id == pluginId } ?: return ""
      val file = File(plugin.installDir, plugin.manifest.jsEntry)
      if (!file.exists()) return ""
      return try {
        // "b64gz:" — gzip + base64 to keep the JS→native bridge payload small
        // (WebView string bridges degrade past ~10 MB). Decompressed in JS via
        // DecompressionStream('gzip').
        val bos = java.io.ByteArrayOutputStream()
        java.util.zip.GZIPOutputStream(bos).use { it.write(file.readBytes()) }
        "b64gz:" + java.util.Base64.getEncoder().encodeToString(bos.toByteArray())
      } catch (e: Exception) {
        android.util.Log.e("ViewIt", "loadPluginBundle failed: ${plugin.manifest.id}", e)
        ""
      }
    }

    @JavascriptInterface
    fun installPluginLocal(zipPath: String, callbackId: String) {
      val pm = (application as? ViewItApp)?.pluginManager ?: return
      Thread {
        try {
          val result = pm.installLocalZip(zipPath) { progress ->
            val msg = JSONObject().apply {
              put("id", callbackId)
              put("event", "progress")
              put("progress", progress)
            }
            runOnUiThread {
              webView.evaluateJavascript("window._pluginCallback && window._pluginCallback($msg)", null)
            }
          }
          val msg = JSONObject().apply {
            put("id", callbackId)
            if (result.isSuccess) {
              put("event", "complete")
            } else {
              put("event", "error")
              put("error", result.exceptionOrNull()?.message ?: "Install failed")
            }
          }
          runOnUiThread {
            webView.evaluateJavascript("window._pluginCallback && window._pluginCallback($msg)", null)
          }
        } catch (e: Exception) {
          android.util.Log.e("ViewIt", "Local plugin install error", e)
          val msg = JSONObject().apply {
            put("id", callbackId)
            put("event", "error")
            put("error", e.message ?: e.javaClass.simpleName)
          }
          runOnUiThread {
            webView.evaluateJavascript("window._pluginCallback && window._pluginCallback($msg)", null)
          }
        }
      }.start()
    }

    @JavascriptInterface
    fun installPlugin(manifestJson: String, callbackId: String) {
      val pm = (application as? ViewItApp)?.pluginManager ?: return
      Thread {
        try {
          val obj = JSONObject(manifestJson)
          val manifest = PluginManager.fetchManifestFromJson(obj)
          val result = pm.installPlugin(manifest) { progress ->
            val msg = JSONObject().apply {
              put("id", callbackId)
              put("event", "progress")
              put("progress", progress)
            }
            runOnUiThread {
              webView.evaluateJavascript(
                "window._pluginCallback && window._pluginCallback($msg)",
                null
              )
            }
          }
          if (result.isSuccess) {
            android.util.Log.i("ViewIt", "Plugin installed: ${manifest.id}")
            val msg = JSONObject().apply {
              put("id", callbackId)
              put("event", "complete")
            }
            runOnUiThread {
              webView.evaluateJavascript(
                "window._pluginCallback && window._pluginCallback($msg)",
                null
              )
            }
          } else {
            val reason = result.exceptionOrNull()?.message ?: "Install failed"
            android.util.Log.e("ViewIt", "Plugin install failed: $reason")
            val msg = JSONObject().apply {
              put("id", callbackId)
              put("event", "error")
              put("error", reason)
            }
            runOnUiThread {
              webView.evaluateJavascript(
                "window._pluginCallback && window._pluginCallback($msg)",
                null
              )
            }
          }
        } catch (e: Exception) {
          android.util.Log.e("ViewIt", "Plugin install error", e)
          val msg = JSONObject().apply {
            put("id", callbackId)
            put("event", "error")
            put("error", e.message ?: e.javaClass.simpleName)
          }
          runOnUiThread {
            webView.evaluateJavascript(
              "window._pluginCallback && window._pluginCallback($msg)",
              null
            )
          }
        }
      }.start()
    }

    @JavascriptInterface
    fun removePlugin(pluginId: String): Boolean {
      val pm = (application as? ViewItApp)?.pluginManager ?: return false
      return pm.removePlugin(pluginId).isSuccess
    }

    @JavascriptInterface
    fun fetchPluginCatalog(callbackId: String) {
      val pm = (application as? ViewItApp)?.pluginManager ?: return
      Thread {
        val result = pm.fetchCatalog()
        val json = if (result.isSuccess) {
          val arr = JSONArray()
          result.getOrNull()?.forEach { m ->
            arr.put(JSONObject().apply {
              put("id", m.id)
              put("name", m.name)
              put("version", m.version)
              put("description", m.description)
              put("entryClass", m.entryClass)
              put("formats", JSONArray(m.supportedFormats))
              put("downloadUrl", m.downloadUrl)
              put("sizeBytes", m.sizeBytes)
              put("installedSizeBytes", m.installedSizeBytes)
              put("checksum", m.checksum)
              put("abi", m.abi)
            })
          }
          arr.toString()
        } else {
          "[]"
        }
        runOnUiThread {
          val escaped = json.replace("\\", "\\\\").replace("'", "\\'").replace("\n", "\\n")
          webView.evaluateJavascript(
            "window._catalogCallback && window._catalogCallback('$callbackId', '$escaped')",
            null
          )
        }
      }.start()
    }

    @JavascriptInterface
    fun fetchPluginCatalogFromUrl(url: String, callbackId: String) {
      val pm = (application as? ViewItApp)?.pluginManager ?: return
      Thread {
        val result = pm.fetchCatalog(url)
        val payload = JSONObject().apply { put("id", callbackId) }
        if (result.isSuccess) {
          val arr = JSONArray()
          result.getOrNull()?.forEach { m ->
            arr.put(JSONObject().apply {
              put("id", m.id)
              put("name", m.name)
              put("version", m.version)
              put("description", m.description)
              put("entryClass", m.entryClass)
              put("formats", JSONArray(m.supportedFormats))
              put("downloadUrl", m.downloadUrl)
              put("sizeBytes", m.sizeBytes)
              put("installedSizeBytes", m.installedSizeBytes)
              put("checksum", m.checksum)
              put("abi", m.abi)
            })
          }
          payload.put("plugins", arr)
        } else {
          payload.put("error", result.exceptionOrNull()?.message ?: "Failed to fetch catalog")
        }
        runOnUiThread {
          webView.evaluateJavascript(
            "window._customCatalogCallback && window._customCatalogCallback($payload)",
            null
          )
        }
      }.start()
    }

    @JavascriptInterface
    fun renderDocumentWithPlugin(pluginId: String, uri: String, ext: String, callbackId: String) {
      val pm = (application as? ViewItApp)?.pluginManager ?: return
      Thread {
        val payload = JSONObject().apply { put("id", callbackId) }
        try {
          val plugin = pm.documentPluginForExt(pluginId, ext)
            ?: throw IllegalArgumentException("No installed document plugin for .$ext")
          val documentPlugin = plugin.documentPlugin
            ?: throw IllegalArgumentException("Plugin $pluginId does not implement document rendering")
          val thread = Thread.currentThread()
          val previousClassLoader = thread.contextClassLoader
          val json = try {
            thread.contextClassLoader = plugin.instance.javaClass.classLoader
            documentPlugin.render(Uri.parse(uri), ext)
          } finally {
            thread.contextClassLoader = previousClassLoader
          }
          payload.put("document", JSONObject(json))
        } catch (e: Throwable) {
          android.util.Log.e("ViewIt", "Document plugin render failed", e)
          payload.put("error", e.message ?: e.javaClass.simpleName)
        }
        runOnUiThread {
          val jsonStr = payload.toString()
          webView.evaluateJavascript(
            "(window._documentPluginCallback || window._docPluginCallback) && (window._documentPluginCallback || window._docPluginCallback)(${jsonStr})",
            null
          )
        }
      }.start()
    }

    // ---- Archive bridge (compression-universal) ---------------------------------

    private fun archivePlugin(pluginId: String): ArchivePlugin {
      val pm = (application as? ViewItApp)?.pluginManager
        ?: throw IllegalStateException("Plugin manager unavailable")
      return pm.documentPluginForId(pluginId)?.documentPlugin as? ArchivePlugin
        ?: throw IllegalArgumentException("Plugin $pluginId is not an archive plugin")
    }

    private fun <T> withArchivePluginLoader(instance: Any, block: () -> T): T {
      val thread = Thread.currentThread()
      val prev = thread.contextClassLoader
      return try {
        thread.contextClassLoader = instance.javaClass.classLoader
        block()
      } finally {
        thread.contextClassLoader = prev
      }
    }

    private fun emitArchiveCallback(payload: JSONObject) {
      runOnUiThread {
        webView.evaluateJavascript(
          "window._pluginArchiveCallback && window._pluginArchiveCallback($payload)",
          null
        )
      }
    }

    @JavascriptInterface
    fun listPluginArchiveAsync(pluginId: String, uri: String, name: String, callbackId: String) {
      Thread {
        val payload = JSONObject().apply {
          put("id", callbackId)
          put("op", "list")
        }
        try {
          val plugin = archivePlugin(pluginId)
          val json = withArchivePluginLoader(plugin) { plugin.listArchive(Uri.parse(uri), name) }
          val obj = JSONObject(json)
          if (obj.optBoolean("ok", false)) payload.put("manifest", obj)
          else payload.put("error", obj.optString("error", "Listing failed"))
        } catch (e: Throwable) {
          android.util.Log.e("ViewIt", "Archive list failed", e)
          payload.put("error", e.message ?: e.javaClass.simpleName)
        }
        emitArchiveCallback(payload)
      }.start()
    }

    @JavascriptInterface
    fun extractPluginArchiveEntryAsync(pluginId: String, uri: String, name: String, entryName: String, callbackId: String) {
      Thread {
        val payload = JSONObject().apply {
          put("id", callbackId)
          put("op", "entry")
        }
        try {
          val plugin = archivePlugin(pluginId)
          val bytes = withArchivePluginLoader(plugin) { plugin.extractEntry(Uri.parse(uri), name, entryName) }
          if (bytes == null) {
            payload.put("error", "Entry is too large for an in-place preview or could not be read — use Save to extract it")
            payload.put("tooLarge", true)
          } else {
            payload.put("base64", java.util.Base64.getEncoder().encodeToString(bytes))
            payload.put("size", bytes.size)
          }
        } catch (e: Throwable) {
          android.util.Log.e("ViewIt", "Archive entry extract failed", e)
          payload.put("error", e.message ?: e.javaClass.simpleName)
        }
        emitArchiveCallback(payload)
      }.start()
    }

    @JavascriptInterface
    fun extractPluginArchiveAllAsync(pluginId: String, uri: String, name: String, callbackId: String) {
      Thread {
        val payload = JSONObject().apply {
          put("id", callbackId)
          put("op", "extract-all")
        }
        try {
          val plugin = archivePlugin(pluginId)
          val root = getExternalFilesDir(null)?.takeIf { it.exists() || it.mkdirs() }
            ?: File(applicationContext.filesDir, "Extracted").also { it.mkdirs() }
            ?: throw IllegalStateException("No writable storage")
          val folder = File(root, "Extracted")
          folder.mkdirs()
          val sub = safeFolderName(name)
          val dest = File(folder, sub)
          dest.mkdirs()
          val json = withArchivePluginLoader(plugin) { plugin.extractAll(Uri.parse(uri), name, dest) }
          val obj = JSONObject(json)
          if (obj.optBoolean("ok", false)) {
            payload.put("result", obj)
            payload.put("dir", dest.absolutePath)
          } else {
            payload.put("error", obj.optString("error", "Extraction failed"))
          }
        } catch (e: Throwable) {
          android.util.Log.e("ViewIt", "Archive extract-all failed", e)
          payload.put("error", e.message ?: e.javaClass.simpleName)
        }
        emitArchiveCallback(payload)
      }.start()
    }

    @JavascriptInterface
    fun savePluginArchiveEntryAsync(pluginId: String, uri: String, name: String, entryName: String, displayName: String, mime: String, callbackId: String) {
      try {
        val plugin = archivePlugin(pluginId)
        val code = saveRequestCode++
        pendingSaves[code] = PendingSave(plugin, uri, name, entryName, displayName, callbackId)
        runOnUiThread {
          try {
            val intent = Intent(Intent.ACTION_CREATE_DOCUMENT).apply {
              addCategory(Intent.CATEGORY_OPENABLE)
              type = mime.ifBlank { "application/octet-stream" }
              putExtra(Intent.EXTRA_TITLE, displayName)
              addFlags(Intent.FLAG_GRANT_WRITE_URI_PERMISSION or Intent.FLAG_GRANT_READ_URI_PERMISSION)
            }
            startActivityForResult(intent, code)
          } catch (e: Throwable) {
            pendingSaves.remove(code)
            emitArchiveCallback(JSONObject().apply {
              put("id", callbackId)
              put("op", "save")
              put("error", e.message ?: e.javaClass.simpleName)
            })
          }
        }
      } catch (e: Throwable) {
        emitArchiveCallback(JSONObject().apply {
          put("id", callbackId)
          put("op", "save")
          put("error", e.message ?: e.javaClass.simpleName)
        })
      }
    }

    private fun safeFolderName(name: String): String {
      var base = name.substringBeforeLast('.', name)
      if (base.isBlank()) base = "archive"
      return base.replace(Regex("[^A-Za-z0-9._-]"), "_").take(48).ifBlank { "archive" }
    }
  }
}
