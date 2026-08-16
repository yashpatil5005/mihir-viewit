package ai.viewit.app

import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.view.View
import android.view.WindowInsets
import android.view.WindowInsetsController
import android.widget.ImageButton
import android.widget.ProgressBar
import android.widget.TextView
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import androidx.media3.common.MediaItem
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.ui.PlayerView
import java.io.File

class VideoPlayerActivity : AppCompatActivity() {

    private var player: ExoPlayer? = null
    private lateinit var playerView: PlayerView
    private lateinit var closeButton: ImageButton
    private lateinit var titleText: TextView
    private lateinit var progressBar: ProgressBar
    private lateinit var statusText: TextView
    private lateinit var pluginButton: ImageButton

    private var originalUri: String = ""
    private var originalExt: String = ""
    private var transcodedFile: File? = null
    private var hasTriedPlugin = false
    private var pluginManager: PluginManager? = null
    private var mediaWorker: MediaWorkerClient? = null
    private var mediaWorkerRequestId: String? = null

    companion object {
        const val EXTRA_URI = "uri"
        const val EXTRA_TITLE = "title"
        const val EXTRA_EXT = "ext"
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_video_player)

        pluginManager = (application as? ViewItApp)?.pluginManager

        playerView = findViewById(R.id.player_view)
        closeButton = findViewById(R.id.btn_close)
        titleText = findViewById(R.id.tv_title)
        progressBar = findViewById(R.id.progress_bar)
        statusText = findViewById(R.id.status_text)
        pluginButton = findViewById(R.id.btn_plugin)

        hideSystemBars()

        closeButton.setOnClickListener { finish() }

        pluginButton.setOnClickListener { openPluginStore() }

        originalUri = intent.getStringExtra(EXTRA_URI) ?: run {
            Toast.makeText(this, "No video URI provided", Toast.LENGTH_SHORT).show()
            finish()
            return
        }
        originalExt = intent.getStringExtra(EXTRA_EXT) ?: ""
        val title = intent.getStringExtra(EXTRA_TITLE) ?: "Video"
        titleText.text = title

        val uri = Uri.parse(originalUri)
        initializePlayer(uri)
    }

    private fun hideSystemBars() {
        window.insetsController?.let { controller ->
            controller.hide(WindowInsets.Type.statusBars() or WindowInsets.Type.navigationBars())
            controller.systemBarsBehavior = WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        }
    }

    private fun initializePlayer(uri: Uri) {
        progressBar.visibility = View.VISIBLE
        statusText.text = "Loading video..."
        statusText.visibility = View.VISIBLE
        pluginButton.visibility = View.GONE

        player = ExoPlayer.Builder(this)
            .build()
            .also { exoPlayer ->
                playerView.player = exoPlayer

                exoPlayer.addListener(object : Player.Listener {
                    override fun onPlaybackStateChanged(playbackState: Int) {
                        when (playbackState) {
                            Player.STATE_READY -> {
                                progressBar.visibility = View.GONE
                                statusText.visibility = View.GONE
                            }
                            Player.STATE_BUFFERING -> {
                                progressBar.visibility = View.VISIBLE
                                statusText.text = "Buffering..."
                                statusText.visibility = View.VISIBLE
                            }
                        }
                    }

                    override fun onPlayerError(error: PlaybackException) {
                        val msg = error.message ?: "Unknown error"
                        android.util.Log.e("VideoPlayer", "Playback error: $msg", error)

                        if (!hasTriedPlugin) {
                            hasTriedPlugin = true
                            tryPluginTranscode(uri)
                        } else {
                            showUnsupported()
                        }
                    }
                })

                val mediaItem = MediaItem.fromUri(uri)
                exoPlayer.setMediaItem(mediaItem)
                exoPlayer.playWhenReady = true
                exoPlayer.prepare()
            }
    }

    private fun tryPluginTranscode(uri: Uri) {
        val pm = pluginManager
        val ext = originalExt.lowercase().trimStart('.')
        val mimeType = getMimeType(ext)

        val plugin = if (mimeType.isNotEmpty()) pm?.canHandle(mimeType) else null
            ?: pm?.canHandleExt(ext)

        if (plugin == null) {
            android.util.Log.i("VideoPlayer", "No plugin for format: $ext")
            showNoPlugin(ext)
            return
        }

        android.util.Log.i("VideoPlayer", "Using plugin ${plugin.manifest.id} for: $ext")
        statusText.text = "Converting with ${plugin.manifest.name}..."
        statusText.visibility = View.VISIBLE
        progressBar.visibility = View.VISIBLE
        pluginButton.visibility = View.GONE

        if (BuildConfig.VIEWIT_MEDIA_WORKER_ENABLED) {
            tryWorkerTranscode(uri, ext)
            return
        }

        Thread {
            try {
                val inputPath = resolveInputPath(uri)
                if (inputPath == null) {
                    runOnUiThread {
                        statusText.text = "Cannot resolve file path"
                    }
                    return@Thread
                }

                val outputDir = File(cacheDir, "transcoded")
                outputDir.mkdirs()
                val outputFile = File(outputDir, "transcoded_${System.currentTimeMillis()}.mp4")

                val pluginInput = if (inputPath.startsWith("http://") || inputPath.startsWith("https://")) {
                    Uri.parse(inputPath)
                } else {
                    Uri.fromFile(File(inputPath))
                }

                val thread = Thread.currentThread()
                val previousClassLoader = thread.contextClassLoader
                thread.contextClassLoader = plugin.instance.javaClass.classLoader
                val success = try {
                    val mediaPlugin = plugin.mediaPlugin ?: throw IllegalStateException("Plugin ${plugin.manifest.id} is not a media plugin")
                    mediaPlugin.transcode(pluginInput, outputFile, object : PluginProgress {
                        override fun update(progress: Float) {
                            val pct = (progress * 100).toInt()
                            runOnUiThread { statusText.text = "Converting... $pct%" }
                        }
                    })
                } finally {
                    thread.contextClassLoader = previousClassLoader
                }

                if (!success || !outputFile.exists() || outputFile.length() == 0L) {
                    runOnUiThread {
                        statusText.text = "Conversion failed"
                        showNoPlugin(ext)
                    }
                    return@Thread
                }

                transcodedFile = outputFile

                runOnUiThread {
                    playTranscodedFile(outputFile)
                }
            } catch (e: Throwable) {
                android.util.Log.e("VideoPlayer", "Plugin transcode error", e)
                runOnUiThread {
                    showPluginFailure(originalExt, e)
                }
            }
        }.start()
    }

    private fun tryWorkerTranscode(uri: Uri, ext: String) {
        Thread {
            val inputPath = resolveInputPath(uri)
            if (inputPath == null) {
                runOnUiThread { showPluginFailure(ext, IllegalStateException("Cannot resolve file path")) }
                return@Thread
            }
            val outputDir = File(cacheDir, "transcoded").apply { mkdirs() }
            val outputFile = File(outputDir, "worker_${System.currentTimeMillis()}.mp4")
            runOnUiThread {
                val client = mediaWorker ?: MediaWorkerClient(this).also { mediaWorker = it }
                mediaWorkerRequestId = client.transcode(File(inputPath), outputFile, ext, object : MediaWorkerClient.Callback {
                    override fun onProgress(progress: Float) {
                        statusText.text = "Converting in isolated worker... ${(progress * 100).toInt()}%"
                    }

                    override fun onComplete(output: File) {
                        mediaWorkerRequestId = null
                        transcodedFile = output
                        playTranscodedFile(output)
                    }

                    override fun onError(error: String) {
                        mediaWorkerRequestId = null
                        showPluginFailure(ext, IllegalStateException(error))
                    }
                })
            }
        }.start()
    }

    private fun playTranscodedFile(outputFile: File) {
        android.util.Log.i("VideoPlayer", "Plugin transcode complete: ${outputFile.absolutePath}")
        player?.release()
        player = null
        player = ExoPlayer.Builder(this)
            .build()
            .also { exoPlayer ->
                playerView.player = exoPlayer
                exoPlayer.addListener(object : Player.Listener {
                    override fun onPlaybackStateChanged(playbackState: Int) {
                        when (playbackState) {
                            Player.STATE_READY -> {
                                progressBar.visibility = View.GONE
                                statusText.visibility = View.GONE
                            }
                            Player.STATE_BUFFERING -> {
                                progressBar.visibility = View.VISIBLE
                                statusText.text = "Buffering..."
                                statusText.visibility = View.VISIBLE
                            }
                        }
                    }

                    override fun onPlayerError(error: PlaybackException) {
                        progressBar.visibility = View.GONE
                        statusText.text = "Playback failed: ${error.message}"
                    }
                })
                exoPlayer.setMediaItem(MediaItem.fromUri(Uri.fromFile(outputFile)))
                exoPlayer.playWhenReady = true
                exoPlayer.prepare()
            }
    }

    private fun showUnsupported() {
        progressBar.visibility = View.GONE
        val ext = originalExt.lowercase().trimStart('.')
        statusText.text = "Format .$ext is not supported by the built-in player"
        pluginButton.visibility = View.VISIBLE
    }

    private fun showNoPlugin(ext: String) {
        progressBar.visibility = View.GONE
        statusText.text = "No plugin installed for .$ext format.\nInstall a plugin to play this format."
        pluginButton.visibility = View.VISIBLE
    }

    private fun showPluginFailure(ext: String, error: Throwable) {
        progressBar.visibility = View.GONE
        val format = ext.lowercase().trimStart('.')
        val reason = error.message ?: error.javaClass.simpleName
        statusText.text = "Plugin failed to convert .$format.\n$reason"
        pluginButton.visibility = View.VISIBLE
    }

    private fun openPluginStore() {
        val intent = Intent(this, MainActivity::class.java).apply {
            action = "ai.viewit.app.PLUGIN_STORE"
            addFlags(Intent.FLAG_ACTIVITY_CLEAR_TOP)
        }
        startActivity(intent)
        finish()
    }

    private fun getMimeType(ext: String): String {
        return when (ext) {
            "mp4", "m4v", "f4v" -> "video/mp4"
            "mkv", "webm" -> "video/x-matroska"
            "avi" -> "video/x-msvideo"
            "mov" -> "video/quicktime"
            "3gp" -> "video/3gpp"
            "flv" -> "video/x-flv"
            "asf", "wmv" -> "video/x-ms-wmv"
            "ts", "m2ts", "mts" -> "video/mp2t"
            "ogv" -> "video/ogg"
            "mp3" -> "audio/mpeg"
            "m4a", "aac" -> "audio/mp4"
            "flac" -> "audio/flac"
            "ogg" -> "audio/ogg"
            "wav" -> "audio/wav"
            "aif", "aiff" -> "audio/aiff"
            "wma" -> "audio/x-ms-wma"
            else -> ""
        }
    }

    private fun resolveInputPath(uri: Uri): String? {
        val scheme = uri.scheme

        if (scheme == "file") return uri.path

        if (scheme == "content") {
            try {
                val cursor = contentResolver.query(uri, null, null, null, null)
                cursor?.use {
                    if (it.moveToFirst()) {
                        val idx = it.getColumnIndex("_data")
                        if (idx >= 0) {
                            val path = it.getString(idx)
                            if (path != null && File(path).exists()) return path
                        }
                    }
                }
            } catch (_: Exception) {}

            try {
                val inputStream = contentResolver.openInputStream(uri) ?: return null
                val tempFile = File(cacheDir, "temp_input_${System.currentTimeMillis()}")
                tempFile.outputStream().use { output -> inputStream.copyTo(output) }
                inputStream.close()
                return tempFile.absolutePath
            } catch (_: Exception) {}
        }

        if (scheme == "http" || scheme == "https") return uri.toString()

        return null
    }

    override fun onStart() {
        super.onStart()
        player?.playWhenReady = true
    }

    override fun onResume() {
        super.onResume()
        hideSystemBars()
        player?.playWhenReady = true
    }

    override fun onPause() {
        super.onPause()
        player?.playWhenReady = false
    }

    override fun onStop() {
        super.onStop()
        player?.release()
        player = null
    }

    override fun onDestroy() {
        super.onDestroy()
        player?.release()
        player = null
        transcodedFile?.delete()
        mediaWorkerRequestId?.let { mediaWorker?.cancel(it) }
        mediaWorker?.close()
        mediaWorker = null
    }
}
