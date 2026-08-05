package ai.viewit.plugins.compressionuniversal

import android.content.Context
import android.net.Uri
import ai.viewit.app.ArchivePlugin
import ai.viewit.app.ViewItDocumentPlugin
import java.io.File
import java.io.FileOutputStream
import java.security.MessageDigest

/**
 * Compression Universal — precises listing of every compression format the
 * app is licensed for (zip, tar, gz, bz2, xz, zst, lz4, lzma, 7z, rar and the
 * tar.* chains) WITHOUT extracting the whole archive first, plus on-demand
 * entry previews and full extraction to a caller-chosen directory.
 *
 * Extraction targets app-scoped storage (no permission needed) and
 * user-selected SAF locations. The catalog entry declares the storage
 * scope so the Plugin Store can surface it.
 */
class CompressionUniversalPlugin : ViewItDocumentPlugin, ArchivePlugin {

    override val id = "compression-universal"
    override val version = "0.1.0"
    override val supportedFormats = listOf(
        "zip", "7z", "rar", "tar",
        "gz", "tgz", "bz2", "tbz2", "xz", "txz", "zst", "tzst",
        "lz4", "lzma", "tlz",
        "tar.gz", "tar.bz2", "tar.xz", "tar.zst", "tar.lz4", "tar.lzma"
    )

    private var initialized = false
    private var ctx: Context? = null

    override fun initialize(context: Context) {
        ctx = context
        if (!initialized) {
            System.loadLibrary("viewit_plugin_compression_universal")
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
        // The archive viewer uses the structured list/extract bridge, not the
        // office-style HTML render contract.
        return """{"kind":"unsupported","format":"$ext","reason":"compression-universal renders via the archive bridge","suggestion":"none"}"""
    }

    override fun cleanup() {
        initialized = false
    }

    /** JSON manifest: {ok, format, entries, notes}. */
    override fun listArchive(uri: Uri, name: String): String {
        val file = materialize(uri) ?: return """{"ok":false,"error":"Unable to read file from URI"}"""
        return nativeListArchive(file.absolutePath, name)
    }

    /** Raw bytes of one entry, or null when it is too large / missing / encrypted. */
    override fun extractEntry(uri: Uri, name: String, entryName: String): ByteArray? {
        val file = materialize(uri) ?: return null
        return nativeExtractEntry(file.absolutePath, name, entryName)
    }

    /** Stream one entry (no size cap) into a destination file; returns {ok, bytes, error}. */
    override fun extractEntryToFile(uri: Uri, name: String, entryName: String, outFile: File): String {
        val file = materialize(uri) ?: return """{"ok":false,"error":"Unable to read file from URI"}"""
        return nativeExtractEntryToFile(file.absolutePath, name, entryName, outFile.absolutePath)
    }

    /** Extract the whole archive into destDir; returns {ok, extracted, bytes, dir, notes}. */
    override fun extractAll(uri: Uri, name: String, destDir: File): String {
        val file = materialize(uri) ?: return """{"ok":false,"error":"Unable to read file from URI"}"""
        return nativeExtractAll(file.absolutePath, name, destDir.absolutePath)
    }

    override fun detectFormat(uri: Uri, name: String): String {
        val file = materialize(uri) ?: return "unknown"
        return nativeDetectFormat(file.absolutePath)
    }

    private fun materialize(uri: Uri): File? {
        val context = ctx ?: return null
        return try {
            if (uri.scheme == "file") {
                val path = uri.path ?: return null
                File(path)
            } else {
                val digest = MessageDigest.getInstance("SHA-256")
                    .digest(uri.toString().toByteArray(Charsets.UTF_8))
                    .take(12)
                    .joinToString("") { "%02x".format(it) }
                val dir = File(context.cacheDir, "viewit_compression").apply { mkdirs() }
                val dest = File(dir, "$digest.bin")
                if (!dest.exists() || dest.length() == 0L) {
                    context.contentResolver.openInputStream(uri)?.use { input ->
                        FileOutputStream(dest).use { output -> input.copyTo(output) }
                    } ?: return null
                }
                dest
            }
        } catch (e: Exception) {
            null
        }
    }

    private external fun nativeCanHandleMimeType(mimeType: String): Boolean
    private external fun nativeCanHandleExt(ext: String): Boolean
    private external fun nativeListArchive(path: String, name: String): String
    private external fun nativeDetectFormat(path: String): String
    private external fun nativeExtractEntry(path: String, name: String, entryName: String): ByteArray?
    private external fun nativeExtractEntryToFile(path: String, name: String, entryName: String, outPath: String): String
    private external fun nativeExtractAll(path: String, name: String, destDir: String): String
}
