package ai.viewit.app

import android.content.Intent
import android.database.Cursor
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.provider.DocumentsContract
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

  /** Editor base "Save as…" flow: text is written after SAF picks a destination. */
  private data class PendingEditorSave(
    val text: String,
    val callbackId: String,
  )
  private val pendingEditorSaves = java.util.concurrent.ConcurrentHashMap<Int, PendingEditorSave>()
  private var editorSaveRequestCode = 9201

  /** Archive "Extract all to folder" flow: SAF ACTION_OPEN_DOCUMENT_TREE. The
   *  archive is unpacked to a cache staging dir, then mirrored into the
   *  user-chosen tree via DocumentsContract on onActivityResult. */
  private data class PendingExtract(
    val plugin: ArchivePlugin,
    val uri: String,
    val name: String,
    val callbackId: String,
  )
  private val pendingExtracts = java.util.concurrent.ConcurrentHashMap<Int, PendingExtract>()
  private var extractRequestCode = 9101

  companion object {
    const val ACTION_DEBUG_OPEN = "ai.viewit.app.action.DEBUG_OPEN"
    const val ACTION_DEBUG_INSTALL_PLUGIN = "ai.viewit.app.action.DEBUG_INSTALL_PLUGIN"
  }

  private fun dispatchEnvelope(webView: WebView, envelope: JSONObject) {
    webView.evaluateJavascript(
      "window.__viewitBridgeDispatch ? window.__viewitBridgeDispatch($envelope) : " +
        "(window.__viewitBridgeQueue = window.__viewitBridgeQueue || []).push($envelope)",
      null,
    )
  }

  private fun dispatchResult(webView: WebView, payload: JSONObject) {
    val failed = payload.has("error") || (payload.has("ok") && !payload.optBoolean("ok"))
    val envelope = JSONObject().apply {
      put("id", payload.getString("id"))
      put("event", if (failed) "error" else "complete")
      if (failed) put("error", payload.optString("error", "Operation failed"))
      else put("result", payload)
    }
    dispatchEnvelope(webView, envelope)
  }

  override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
    super.onActivityResult(requestCode, resultCode, data)
    val webView = bridgeWebView ?: return
    pendingEditorSaves.remove(requestCode)?.let { pending ->
      val payload = JSONObject().apply { put("id", pending.callbackId) }
      if (resultCode != RESULT_OK || data?.data == null) {
        payload.put("ok", false)
        payload.put("error", "Save cancelled")
      } else {
        try {
          val bytes = pending.text.toByteArray(Charsets.UTF_8)
          contentResolver.openOutputStream(data.data!!)?.use { it.write(bytes) } ?: error("No writable stream")
          payload.put("ok", true)
          payload.put("size", bytes.size)
        } catch (e: Throwable) {
          payload.put("ok", false)
          payload.put("error", e.message ?: e.javaClass.simpleName)
        }
      }
      webView.post {
        dispatchResult(webView, payload)
      }
      return
    }
    pendingExtracts.remove(requestCode)?.let { pending ->
      val payload = JSONObject().apply {
        put("id", pending.callbackId)
        put("op", "extract-all")
      }
      if (resultCode != RESULT_OK || data?.data == null) {
        payload.put("ok", false)
        payload.put("error", "Extract cancelled")
        webView.post {
          dispatchResult(webView, payload)
        }
        return
      }
      val treeUri = data.data!!
      Thread {
        try {
          val stage = File(applicationContext.cacheDir, "extract_stage_$requestCode")
          val resultJson = withClassLoader(pending.plugin) {
            pending.plugin.extractAll(Uri.parse(pending.uri), pending.name, stage)
          }
          val result = JSONObject(resultJson)
          if (!result.optBoolean("ok", false)) {
            payload.put("ok", false)
            payload.put("error", result.optString("error", "Extraction failed"))
          } else {
            val count = mirrorToTree(treeUri, stage)
            payload.put("ok", true)
            payload.put("count", count)
            payload.put("dir", treePath(treeUri))
          }
          stage.deleteRecursively()
        } catch (e: Throwable) {
          payload.put("ok", false)
          payload.put("error", e.message ?: e.javaClass.simpleName)
        }
        webView.post {
          dispatchResult(webView, payload)
        }
      }.start()
      return
    }
    val pending = pendingSaves.remove(requestCode) ?: return
    val payload = JSONObject().apply {
      put("id", pending.callbackId)
      put("op", "save")
    }
    if (resultCode != RESULT_OK || data?.data == null) {
      payload.put("ok", false)
      payload.put("error", "Save cancelled")
      webView.post {
        dispatchResult(webView, payload)
      }
      return
    }
    val outUri = data.data!!
    Thread {
      try {
        val tmp = File(applicationContext.cacheDir, "save_stage_$requestCode")
        val resultJson = withClassLoader(pending.plugin) {
          pending.plugin.extractEntryToFile(Uri.parse(pending.uri), pending.name, pending.entryName, tmp)
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
      webView.post {
        dispatchResult(webView, payload)
      }
    }.start()
  }

  /** Run a plugin call with its own classloader on the thread's context. */
  private fun <T> withClassLoader(plugin: ArchivePlugin, block: () -> T): T {
    val thread = Thread.currentThread()
    val prev = thread.contextClassLoader
    return try {
      thread.contextClassLoader = plugin.javaClass.classLoader
      block()
    } finally {
      thread.contextClassLoader = prev
    }
  }

  /** Copy a stage directory tree into an SAF tree URI (no storage permission). */
  private fun mirrorToTree(treeUri: Uri, stage: File): Int {
    var copied = 0
    val rootId = DocumentsContract.getTreeDocumentId(treeUri)
    val rootUri = DocumentsContract.buildDocumentUriUsingTree(treeUri, rootId)
    fun copyInto(parentUri: Uri, dir: File) {
      val children = dir.listFiles() ?: return
      for (child in children) {
        val doc = DocumentsContract.createDocument(
          contentResolver,
          parentUri,
          if (child.isDirectory) DocumentsContract.Document.MIME_TYPE_DIR else mimeOf(child.name),
          child.name
        ) ?: continue
        if (child.isDirectory) {
          copyInto(doc, child)
        } else {
          contentResolver.openOutputStream(doc)?.use { out ->
            child.inputStream().use { it.copyTo(out) }
            copied += 1
          }
        }
      }
    }
    copyInto(rootUri, stage)
    return copied
  }

  private fun mimeOf(name: String): String = when (name.substringAfterLast('.', "").lowercase()) {
    "json" -> "application/json"
    "html", "htm" -> "text/html"
    "txt", "md", "csv", "log", "xml" -> "text/plain"
    "png" -> "image/png"
    "jpg", "jpeg" -> "image/jpeg"
    "gif" -> "image/gif"
    "pdf" -> "application/pdf"
    else -> "application/octet-stream"
  }

  /** Human-readable path like "Download/MyFolder" for the chosen SAF tree. */
  private fun treePath(treeUri: Uri): String =
    DocumentsContract.getTreeDocumentId(treeUri).substringAfter(":").replace("%2F", "/")

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    WebView.setWebContentsDebuggingEnabled(
      RuntimeBuildPolicy.enablesWebViewDebugging(BuildConfig.VIEWIT_APP_PROFILE)
    )
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
    val debugIntentsEnabled = RuntimeBuildPolicy.enablesDebugIntents(BuildConfig.VIEWIT_APP_PROFILE)
    val isDebugOpen = debugIntentsEnabled && action == ACTION_DEBUG_OPEN
    val isDebugInstall = debugIntentsEnabled && action == ACTION_DEBUG_INSTALL_PLUGIN
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

  private fun pluginProvidersJson(providers: List<PluginProviderManifest>): JSONArray = JSONArray().apply {
    providers.forEach { provider ->
      put(JSONObject().apply {
        put("id", provider.id)
        put("packageId", provider.packageId)
        put("service", provider.service)
        put("contractVersion", provider.contractVersion)
        put("runtime", provider.runtime)
        put("trustClass", provider.trustClass)
        if (provider.formats.isNotEmpty()) put("formats", JSONArray(provider.formats))
        if (provider.mimeTypes.isNotEmpty()) put("mimeTypes", JSONArray(provider.mimeTypes))
        if (provider.priority != 0) put("priority", provider.priority)
        if (provider.hostGrants.isNotEmpty()) put("hostGrants", JSONArray(provider.hostGrants))
      })
    }
  }

  private fun providerHealthJson(pm: PluginManager, manifest: PluginManifest): JSONObject = JSONObject().apply {
    val providerIds = manifest.providers.map { it.id }.ifEmpty { listOf(manifest.id) }
    providerIds.forEach { id ->
      val health = pm.providerHealth.get(id)
      put(id, JSONObject().apply {
        put("state", health.state)
        put("consecutiveFailures", health.consecutiveFailures)
        put("totalFailures", health.totalFailures)
        health.lastFailureKind?.let { put("lastFailureKind", it) }
        put("updatedAt", health.updatedAt)
      })
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
    private fun dispatchBridge(payload: JSONObject) {
      dispatchEnvelope(webView, payload)
    }

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
          put("abiVersion", p.manifest.abiVersion)
          put("minAppVersion", p.manifest.minAppVersion)
          put("capabilities", JSONArray(p.manifest.capabilities))
          put("runtime", p.manifest.runtime.ifBlank { "native" })
          put("schemaVersion", p.manifest.schemaVersion)
          if (p.manifest.publisher.isNotEmpty()) put("publisher", p.manifest.publisher)
          if (p.manifest.providers.isNotEmpty()) put("providers", pluginProvidersJson(p.manifest.providers))
          put("providerHealth", providerHealthJson(pm, p.manifest))
          if (p.manifest.base.isNotEmpty() && p.manifest.base != "view") put("base", p.manifest.base)
          if (p.manifest.runtime == "js" || p.manifest.jsEntry.isNotBlank()) put("jsEntry", p.manifest.jsEntry)
          if (p.manifest.cssEntry.isNotEmpty()) put("cssEntry", p.manifest.cssEntry)
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
      return try {
        val file = PluginRuntimePolicy.zipEntryDestination(plugin.installDir, plugin.manifest.jsEntry)
        if (!file.isFile) return ""
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
      if (!RuntimeBuildPolicy.enablesLocalPluginInstall(BuildConfig.VIEWIT_APP_PROFILE)) {
        val msg = JSONObject().apply {
          put("id", callbackId)
          put("event", "error")
          put("error", "Local plugin installation is disabled in production builds")
        }
        runOnUiThread {
          dispatchBridge(msg)
        }
        return
      }
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
              dispatchBridge(msg)
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
            dispatchBridge(msg)
          }
        } catch (e: Exception) {
          android.util.Log.e("ViewIt", "Local plugin install error", e)
          val msg = JSONObject().apply {
            put("id", callbackId)
            put("event", "error")
            put("error", e.message ?: e.javaClass.simpleName)
          }
          runOnUiThread {
            dispatchBridge(msg)
          }
        }
      }.start()
    }

    @JavascriptInterface
    fun restartApp() {
      // A freshly downloaded native-plugin update can't reload an already-open
      // .so in-process; cold-restart so the new version is loaded on next run.
      runOnUiThread {
        try {
          val i = packageManager.getLaunchIntentForPackage(packageName)
          if (i != null) {
            i.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TASK)
            startActivity(i)
          }
        } catch (e: Exception) {
          android.util.Log.e("ViewIt", "restartApp launch failed", e)
        }
        Runtime.getRuntime().exit(0)
      }
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
              dispatchBridge(msg)
            }
          }
          if (result.isSuccess) {
            android.util.Log.i("ViewIt", "Plugin installed: ${manifest.id}")
            val msg = JSONObject().apply {
              put("id", callbackId)
              put("event", "complete")
            }
            runOnUiThread {
              dispatchBridge(msg)
            }
            } else {
              val failure = result.exceptionOrNull()
              val reason = result.exceptionOrNull()?.message ?: "Install failed"
            android.util.Log.e("ViewIt", "Plugin install failed: $reason")
            val msg = JSONObject().apply {
              put("id", callbackId)
              put("event", if (failure is PluginManager.RestartRequiredException) "restart-required" else "error")
              put("error", reason)
            }
            runOnUiThread {
              dispatchBridge(msg)
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
            dispatchBridge(msg)
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
    fun recordProviderFailure(pluginId: String, kind: String) {
      val allowed = setOf("activation", "execution", "timeout", "invalid-output", "cleanup", "transport", "runtime-crash")
      if (kind !in allowed) return
      (application as? ViewItApp)?.pluginManager?.recordProviderFailure(pluginId, kind)
    }

    @JavascriptInterface
    fun fetchPluginCatalog(callbackId: String) {
      val pm = (application as? ViewItApp)?.pluginManager ?: return
      Thread {
        val result = pm.fetchCatalog()
        val plugins = if (result.isSuccess) {
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
              put("abiVersion", m.abiVersion)
              put("minAppVersion", m.minAppVersion)
              put("base", m.base)
              put("capabilities", JSONArray(m.capabilities))
              put("runtime", m.runtime.ifBlank { "native" })
              put("schemaVersion", m.schemaVersion)
              if (m.publisher.isNotEmpty()) put("publisher", m.publisher)
              if (m.providers.isNotEmpty()) put("providers", pluginProvidersJson(m.providers))
              if (m.jsEntry.isNotBlank()) put("jsEntry", m.jsEntry)
              if (m.cssEntry.isNotBlank()) put("cssEntry", m.cssEntry)
            })
          }
          arr
        } else {
          JSONArray()
        }
        runOnUiThread {
          val payload = JSONObject().apply {
            put("id", callbackId)
            if (result.isSuccess) {
              put("event", "complete")
              put("result", plugins)
            } else {
              put("event", "error")
              put("error", result.exceptionOrNull()?.message ?: "Failed to fetch catalog")
            }
          }
          dispatchBridge(payload)
        }
      }.start()
    }

    @JavascriptInterface
    fun fetchPluginCatalogFromUrl(url: String, callbackId: String) {
      val pm = (application as? ViewItApp)?.pluginManager ?: return
      Thread {
        val result = pm.fetchCatalog(url)
        val plugins = JSONArray()
        if (result.isSuccess) {
          result.getOrNull()?.forEach { m ->
            plugins.put(JSONObject().apply {
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
              put("abiVersion", m.abiVersion)
              put("minAppVersion", m.minAppVersion)
              put("base", m.base)
              put("capabilities", JSONArray(m.capabilities))
              put("runtime", m.runtime.ifBlank { "native" })
              put("schemaVersion", m.schemaVersion)
              if (m.publisher.isNotEmpty()) put("publisher", m.publisher)
              if (m.providers.isNotEmpty()) put("providers", pluginProvidersJson(m.providers))
              if (m.jsEntry.isNotBlank()) put("jsEntry", m.jsEntry)
              if (m.cssEntry.isNotBlank()) put("cssEntry", m.cssEntry)
            })
          }
        }
        runOnUiThread {
          val payload = JSONObject().apply {
            put("id", callbackId)
            if (result.isSuccess) {
              put("event", "complete")
              put("result", plugins)
            } else {
              put("event", "error")
              put("error", result.exceptionOrNull()?.message ?: "Failed to fetch catalog")
            }
          }
          dispatchBridge(payload)
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
          pm.recordProviderFailure(pluginId, "execution")
          payload.put("error", e.message ?: e.javaClass.simpleName)
        }
        runOnUiThread {
          dispatchResult(webView, payload)
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
        dispatchResult(webView, payload)
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
    fun detectPluginArchiveFormatAsync(pluginId: String, uri: String, name: String, callbackId: String) {
      Thread {
        val payload = JSONObject().apply {
          put("id", callbackId)
          put("op", "detect")
        }
        try {
          val plugin = archivePlugin(pluginId)
          val format = withArchivePluginLoader(plugin) { plugin.detectFormat(Uri.parse(uri), name) }
          payload.put("format", format.ifBlank { "unknown" })
        } catch (e: Throwable) {
          android.util.Log.e("ViewIt", "Archive format detect failed", e)
          payload.put("format", "unknown")
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
    fun extractPluginArchiveAllToFolderAsync(pluginId: String, uri: String, name: String, callbackId: String) {
      try {
        val plugin = archivePlugin(pluginId)
        val code = extractRequestCode++
        pendingExtracts[code] = PendingExtract(plugin, uri, name, callbackId)
        runOnUiThread {
          try {
            val intent = Intent(Intent.ACTION_OPEN_DOCUMENT_TREE).apply {
              addFlags(Intent.FLAG_GRANT_WRITE_URI_PERMISSION or Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION)
            }
            startActivityForResult(intent, code)
          } catch (e: Throwable) {
            pendingExtracts.remove(code)
            emitArchiveCallback(JSONObject().apply {
              put("id", callbackId)
              put("op", "extract-all")
              put("error", e.message ?: e.javaClass.simpleName)
            })
          }
        }
      } catch (e: Throwable) {
        emitArchiveCallback(JSONObject().apply {
          put("id", callbackId)
          put("op", "extract-all")
          put("error", e.message ?: e.javaClass.simpleName)
        })
      }
    }

    @JavascriptInterface
    fun saveEditedText(text: String, displayName: String, mime: String, callbackId: String) {
      val code = editorSaveRequestCode++
      pendingEditorSaves[code] = PendingEditorSave(text, callbackId)
      runOnUiThread {
        try {
          val intent = Intent(Intent.ACTION_CREATE_DOCUMENT).apply {
            addCategory(Intent.CATEGORY_OPENABLE)
            type = mime.ifBlank { "text/plain" }
            putExtra(Intent.EXTRA_TITLE, displayName)
            addFlags(Intent.FLAG_GRANT_WRITE_URI_PERMISSION or Intent.FLAG_GRANT_READ_URI_PERMISSION)
          }
          startActivityForResult(intent, code)
        } catch (e: Throwable) {
          pendingEditorSaves.remove(code)
          val payload = JSONObject().apply {
            put("id", callbackId)
            put("ok", false)
            put("error", e.message ?: e.javaClass.simpleName)
          }
          dispatchResult(webView, payload)
        }
      }
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
  }
}
