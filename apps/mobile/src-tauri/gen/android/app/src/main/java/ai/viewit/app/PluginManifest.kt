package ai.viewit.app

data class PluginManifest(
    val id: String,
    val name: String,
    val version: String,
    val description: String,
    val minAppVersion: Int,
    val entryClass: String,
    val supportedFormats: List<String>,
    val downloadUrl: String,
    val sizeBytes: Long,
    val installedSizeBytes: Long,
    val checksum: String,
    val abi: String,
)
