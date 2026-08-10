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
    val abiVersion: Int = 1,
    val capabilities: List<String> = emptyList(),
    /** Plugin base the add-on provides: "view" (default) | "play" | "edit" | "tool".
     *  A base is a whole experience a plugin can replace (e.g. a custom media
     *  player) or enrich beyond the built-in viewer. See docs/PLUGIN-BASES.md. */
    val base: String = "view",
    val runtime: String = "",
    /** Relative path of the bundle entry inside the plugin dir for runtime=js plugins. */
    val jsEntry: String = "web/index.js",
    /** Optional relative path of a CSS asset for runtime=js plugins. */
    val cssEntry: String = "",
)
