package ai.viewit.app

import android.content.Context
import android.net.Uri
import java.io.File

interface ViewItPlugin {
    val id: String
    val version: String
    val supportedFormats: List<String>

    fun initialize(context: Context)
    fun canHandle(mimeType: String): Boolean
    fun canHandleExt(ext: String): Boolean
    fun transcode(input: Uri, output: File, onProgress: PluginProgress): Boolean
    fun cleanup()
}
