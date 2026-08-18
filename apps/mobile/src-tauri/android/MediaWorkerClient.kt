package ai.viewit.app

import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.ServiceConnection
import android.os.Bundle
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.os.Message
import android.os.Messenger
import java.io.File
import java.util.UUID
import java.util.concurrent.ConcurrentHashMap

class MediaWorkerClient(private val context: Context) {
    interface Callback {
        fun onProgress(progress: Float)
        fun onComplete(output: File)
        fun onError(error: String)
    }

    private data class Pending(val output: File, val callback: Callback)
    private val pending = ConcurrentHashMap<String, Pending>()
    private var worker: Messenger? = null
    private var binding = false
    private val waiting = mutableListOf<() -> Unit>()
    private val replies = Messenger(ReplyHandler(Looper.getMainLooper()))

    fun transcode(input: File, output: File, ext: String, callback: Callback): String {
        val requestId = UUID.randomUUID().toString()
        pending[requestId] = Pending(output, callback)
        withWorker {
            val message = Message.obtain(null, MediaWorkerProtocol.MSG_TRANSCODE).apply {
                replyTo = replies
                data = Bundle().apply {
                    putString(MediaWorkerProtocol.KEY_REQUEST_ID, requestId)
                    putString(MediaWorkerProtocol.KEY_INPUT_PATH, input.absolutePath)
                    putString(MediaWorkerProtocol.KEY_OUTPUT_PATH, output.absolutePath)
                    putString(MediaWorkerProtocol.KEY_EXT, ext)
                }
            }
            runCatching { worker?.send(message) }
                .onFailure { finishError(requestId, it.message ?: "Media worker unavailable") }
        }
        return requestId
    }

    fun cancel(requestId: String) {
        pending.remove(requestId)?.callback?.onError("Media worker request cancelled")
        runCatching {
            worker?.send(Message.obtain(null, MediaWorkerProtocol.MSG_CANCEL).apply {
                data = Bundle().apply { putString(MediaWorkerProtocol.KEY_REQUEST_ID, requestId) }
            })
        }
    }

    fun testDelay(delayMs: Long, callback: Callback): String {
        val requestId = UUID.randomUUID().toString()
        val output = File(context.cacheDir, "media-worker-probe/delay")
        pending[requestId] = Pending(output, callback)
        withWorker {
            runCatching {
                worker?.send(Message.obtain(null, MediaWorkerProtocol.MSG_TEST_DELAY).apply {
                    replyTo = replies
                    data = Bundle().apply {
                        putString(MediaWorkerProtocol.KEY_REQUEST_ID, requestId)
                        putLong(MediaWorkerProtocol.KEY_DELAY_MS, delayMs)
                    }
                })
            }.onFailure { finishError(requestId, it.message ?: "Media worker unavailable") }
        }
        return requestId
    }

    fun close() {
        pending.keys.toList().forEach(::cancel)
        if (worker != null || binding) runCatching { context.unbindService(connection) }
        worker = null
        binding = false
    }

    private fun withWorker(action: () -> Unit) {
        if (worker != null) {
            action()
            return
        }
        synchronized(waiting) { waiting.add(action) }
        if (binding) return
        binding = true
        val bound = context.bindService(
            Intent(context, MediaWorkerService::class.java),
            connection,
            Context.BIND_AUTO_CREATE,
        )
        if (!bound) failAll("Could not bind media worker")
    }

    private val connection = object : ServiceConnection {
        override fun onServiceConnected(name: ComponentName?, binder: IBinder?) {
            worker = Messenger(binder)
            binding = false
            val actions = synchronized(waiting) { waiting.toList().also { waiting.clear() } }
            actions.forEach { it() }
        }

        override fun onServiceDisconnected(name: ComponentName?) {
            worker = null
            binding = false
            failAll("Media worker process disconnected")
        }

        override fun onBindingDied(name: ComponentName?) = onServiceDisconnected(name)
        override fun onNullBinding(name: ComponentName?) = onServiceDisconnected(name)
    }

    private inner class ReplyHandler(looper: Looper) : Handler(looper) {
        override fun handleMessage(message: Message) {
            val requestId = message.data.getString(MediaWorkerProtocol.KEY_REQUEST_ID).orEmpty()
            val request = pending[requestId] ?: return
            val pluginId = message.data.getString(MediaWorkerProtocol.KEY_PLUGIN_ID)
            when (message.what) {
                MediaWorkerProtocol.MSG_PROGRESS -> request.callback.onProgress(
                    message.data.getFloat(MediaWorkerProtocol.KEY_PROGRESS),
                )
                MediaWorkerProtocol.MSG_COMPLETE -> {
                    pending.remove(requestId)
                    pluginId?.let(::recordSuccess)
                    request.callback.onComplete(request.output)
                }
                MediaWorkerProtocol.MSG_ERROR -> {
                    pluginId?.let(::recordFailure)
                    finishError(
                        requestId,
                        message.data.getString(MediaWorkerProtocol.KEY_ERROR) ?: "Media worker failed",
                    )
                }
            }
        }
    }

    private fun finishError(requestId: String, error: String) {
        pending.remove(requestId)?.callback?.onError(error)
    }

    private fun failAll(error: String) {
        val values = pending.values.toList()
        pending.clear()
        synchronized(waiting) { waiting.clear() }
        values.forEach { it.callback.onError(error) }
    }

    private fun recordSuccess(pluginId: String) {
        (context.applicationContext as? ViewItApp)?.pluginManager?.recordProviderSuccess(pluginId)
    }

    private fun recordFailure(pluginId: String) {
        (context.applicationContext as? ViewItApp)?.pluginManager?.recordProviderFailure(pluginId, "execution")
    }
}
