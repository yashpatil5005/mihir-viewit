package ai.viewit.app

import android.app.Service
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.os.Message
import android.os.Messenger
import android.os.Process
import android.util.Log
import java.io.File
import java.util.concurrent.ConcurrentHashMap

class MediaWorkerService : Service() {
    private val operations = ConcurrentHashMap<String, Thread>()
    private val messenger = Messenger(IncomingHandler(Looper.getMainLooper()))

    override fun onBind(intent: Intent?): IBinder = messenger.binder

    override fun onCreate() {
        super.onCreate()
        Log.i("ViewItMediaWorker", "started pid=${Process.myPid()}")
    }

    private inner class IncomingHandler(looper: Looper) : Handler(looper) {
        override fun handleMessage(message: Message) {
            when (message.what) {
                MediaWorkerProtocol.MSG_TRANSCODE -> startTranscode(message.data, message.replyTo)
                MediaWorkerProtocol.MSG_CANCEL -> cancel(message.data.getString(MediaWorkerProtocol.KEY_REQUEST_ID).orEmpty())
                MediaWorkerProtocol.MSG_TEST_DELAY -> startDelay(message.data, message.replyTo)
                else -> super.handleMessage(message)
            }
        }
    }

    private fun startDelay(data: Bundle, replyTo: Messenger?) {
        val requestId = data.getString(MediaWorkerProtocol.KEY_REQUEST_ID).orEmpty()
        val delayMs = data.getLong(MediaWorkerProtocol.KEY_DELAY_MS).coerceIn(100, 30_000)
        if (replyTo == null || !requestId.matches(Regex("[A-Za-z0-9._-]{1,128}"))) return
        val thread = Thread({
            try {
                Thread.sleep(delayMs)
                replyTo.sendResult(MediaWorkerProtocol.MSG_COMPLETE, requestId, null)
            } catch (_: InterruptedException) {
            } finally {
                operations.remove(requestId)
            }
        }, "viewit-media-delay-$requestId")
        operations[requestId] = thread
        thread.start()
    }

    private fun startTranscode(data: Bundle, replyTo: Messenger?) {
        val requestId = data.getString(MediaWorkerProtocol.KEY_REQUEST_ID).orEmpty()
        val inputPath = data.getString(MediaWorkerProtocol.KEY_INPUT_PATH).orEmpty()
        val outputPath = data.getString(MediaWorkerProtocol.KEY_OUTPUT_PATH).orEmpty()
        val ext = data.getString(MediaWorkerProtocol.KEY_EXT).orEmpty()
        Log.i("ViewItMediaWorker", "request=$requestId ext=$ext pid=${Process.myPid()}")
        if (replyTo == null || !MediaWorkerProtocol.validRequest(requestId, inputPath, outputPath, ext)) {
            replyTo?.sendResult(MediaWorkerProtocol.MSG_ERROR, requestId, "Invalid media worker request")
            return
        }
        if (operations.containsKey(requestId)) {
            replyTo.sendResult(MediaWorkerProtocol.MSG_ERROR, requestId, "Duplicate media worker request")
            return
        }
        val thread = Thread({
            try {
                val input = File(inputPath).canonicalFile
                val output = File(outputPath).canonicalFile
                val cache = cacheDir.canonicalFile
                if (!input.isFile) error("Worker input is not a file")
                if (!output.path.startsWith(cache.path + File.separator)) error("Worker output must be app cache")
                output.parentFile?.mkdirs()
                output.delete()

                val manager = (application as ViewItApp).pluginManager
                val plugin = manager.canHandleExt(ext)
                    ?: error("No media provider installed for .$ext")
                val mediaPlugin = plugin.mediaPlugin ?: error("Provider does not implement media conversion")
                val current = Thread.currentThread()
                val previous = current.contextClassLoader
                current.contextClassLoader = plugin.instance.javaClass.classLoader
                val success = try {
                    mediaPlugin.transcode(Uri.fromFile(input), output, object : PluginProgress {
                        override fun update(progress: Float) {
                            replyTo.sendProgress(requestId, progress)
                        }
                    })
                } finally {
                    current.contextClassLoader = previous
                }
                if (!success || !output.isFile || output.length() == 0L) error("Media conversion failed")
                replyTo.sendResult(MediaWorkerProtocol.MSG_COMPLETE, requestId, null)
            } catch (error: Throwable) {
                replyTo.sendResult(MediaWorkerProtocol.MSG_ERROR, requestId, error.message ?: error.javaClass.simpleName)
            } finally {
                operations.remove(requestId)
            }
        }, "viewit-media-$requestId")
        operations[requestId] = thread
        thread.start()
    }

    private fun cancel(requestId: String) {
        if (operations.remove(requestId) != null) {
            Log.w("ViewItMediaWorker", "hard-cancelling request=$requestId pid=${Process.myPid()}")
            // FFmpeg/native providers are not guaranteed to observe interruption.
            // Terminating this dedicated process is the hard cancellation boundary.
            Process.killProcess(Process.myPid())
        }
    }

    private fun Messenger.sendProgress(requestId: String, progress: Float) {
        send(Message.obtain(null, MediaWorkerProtocol.MSG_PROGRESS).apply {
            data = Bundle().apply {
                putString(MediaWorkerProtocol.KEY_REQUEST_ID, requestId)
                putFloat(MediaWorkerProtocol.KEY_PROGRESS, progress.coerceIn(0f, 1f))
            }
        })
    }

    private fun Messenger.sendResult(what: Int, requestId: String, error: String?) {
        runCatching {
            send(Message.obtain(null, what).apply {
                data = Bundle().apply {
                    putString(MediaWorkerProtocol.KEY_REQUEST_ID, requestId)
                    error?.let { putString(MediaWorkerProtocol.KEY_ERROR, it) }
                }
            })
        }
    }
}
