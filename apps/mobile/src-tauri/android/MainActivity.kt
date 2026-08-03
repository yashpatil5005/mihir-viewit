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

  companion object {
    const val ACTION_DEBUG_OPEN = "ai.viewit.app.action.DEBUG_OPEN"
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
    if (
      !isDebugOpen &&
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
        })
      }
      return arr.toString()
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
  }
}
