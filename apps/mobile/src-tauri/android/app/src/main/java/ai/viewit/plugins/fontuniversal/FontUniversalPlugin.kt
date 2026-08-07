package ai.viewit.plugins.fontuniversal

import android.content.Context
import android.net.Uri
import ai.viewit.app.ViewItDocumentPlugin

class FontUniversalPlugin : ViewItDocumentPlugin {

    override val id = "font-universal"
    override val version = "0.1.0"
    override val supportedFormats = listOf(
        "ttf", "otf", "woff", "woff2", "ttc", "pfb", "cff", "dfont", "sfd", "ps"
    )

    private var initialized = false
    private var ctx: Context? = null

    override fun initialize(context: Context) {
        ctx = context
        if (!initialized) {
            System.loadLibrary("viewit_plugin_font_universal")
            initialized = true
        }
    }

    override fun canHandle(mimeType: String): Boolean {
        return nativeCanHandleMimeType(mimeType)
    }

    override fun canHandleExt(ext: String): Boolean {
        return nativeCanHandleExt(ext)
    }

    override fun render(input: Uri, ext: String): String {
        val context = ctx ?: return unsupportedJson(ext, "Plugin not initialized")
        val bytes = try {
            if (input.scheme == "content") {
                context.contentResolver.openInputStream(input)?.use { it.readBytes() }
            } else {
                val path = input.path ?: throw IllegalArgumentException("Invalid URI path")
                java.io.File(path).readBytes()
            }
        } catch (e: Exception) {
            null
        }
        if (bytes == null || bytes.isEmpty()) {
            return unsupportedJson(ext, "Unable to read file from URI")
        }
        return nativeRenderBytes(bytes, ext.lowercase())
    }

    override fun cleanup() {
        nativeCleanup()
        initialized = false
    }

    private external fun nativeCanHandleMimeType(mimeType: String): Boolean
    private external fun nativeCanHandleExt(ext: String): Boolean
    private external fun nativeRender(filePath: String, ext: String): String
    private external fun nativeRenderBytes(bytes: ByteArray, ext: String): String
    private external fun nativeCleanup()

    private fun unsupportedJson(ext: String, reason: String): String {
        return """{"kind":"unsupported","format":"$ext","reason":"$reason","suggestion":"none"}"""
    }
}
