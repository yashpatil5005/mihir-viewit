package ai.viewit.app

import android.content.Context
import android.net.Uri

interface ViewItDocumentPlugin {
    val id: String
    val version: String
    val supportedFormats: List<String>

    fun initialize(context: Context)
    fun canHandle(mimeType: String): Boolean
    fun canHandleExt(ext: String): Boolean
    fun render(input: Uri, ext: String): String
    fun cleanup()
}
