package ai.viewit.app

import android.net.Uri
import java.io.File

/**
 * Optional capability of a plugin that understands archives (see
 * compression-universal). Distinct from [ViewItDocumentPlugin] so the office
 * bridge stays untouched; plugins advertise archive support by implementing
 * BOTH interfaces. All methods return/consume the same JSON shapes the WebView
 * bridge exchanges, and are safe to call off the main thread.
 */
interface ArchivePlugin {
    /** JSON manifest: {ok, format, entries[], notes[]}. */
    fun listArchive(uri: Uri, name: String): String

    /** Raw bytes of one entry (null when missing / too large / encrypted). */
    fun extractEntry(uri: Uri, name: String, entryName: String): ByteArray?

    /** Stream one entry into outFile; returns {ok, bytes, error}. */
    fun extractEntryToFile(uri: Uri, name: String, entryName: String, outFile: File): String

    /** Extract the whole archive into destDir; returns {ok, extracted, bytes, dir, notes[]}. */
    fun extractAll(uri: Uri, name: String, destDir: File): String

    /** Detected container id ("zip", "7z", …) or "unknown". */
    fun detectFormat(uri: Uri, name: String): String
}
