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

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    WebView.setWebContentsDebuggingEnabled(true)
    super.onCreate(savedInstanceState)
    consumeIncomingIntent(intent)
  }

  override fun onWebViewCreate(webView: WebView) {
    super.onWebViewCreate(webView)
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
    if (
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

    if (lines.isEmpty()) return

    try {
      val f = File(applicationContext.filesDir, "viewit_pending_opens.txt")
      f.writeText(lines.joinToString("\n") + "\n")
    } catch (e: Exception) {
      android.util.Log.e("ViewIt", "pending opens write failed", e)
    }
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
    val projection = arrayOf(MediaStore.MediaColumns.DISPLAY_NAME)
    val cursor: Cursor? = contentResolver.query(uri, projection, null, null, null)
    return cursor?.use {
      if (it.moveToFirst()) {
        val idx = it.getColumnIndex(MediaStore.MediaColumns.DISPLAY_NAME)
        if (idx >= 0) it.getString(idx) else null
      } else null
    }
  }

  inner class AndroidBridge(private val webView: WebView) {
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
          } else {
            android.util.Log.e("ViewIt", "Plugin install failed: ${result.exceptionOrNull()?.message}")
          }
        } catch (e: Exception) {
          android.util.Log.e("ViewIt", "Plugin install error", e)
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
          webView.evaluateJavascript(
            "window._documentPluginCallback && window._documentPluginCallback($payload)",
            null
          )
        }
      }.start()
    }
  }
}
