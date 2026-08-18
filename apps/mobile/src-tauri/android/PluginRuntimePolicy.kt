package ai.viewit.app

import java.io.File

object PluginRuntimePolicy {
    const val SUPPORTED_ABI_VERSION = 1
    const val MAX_PLUGIN_ENTRIES = 10_000
    const val MAX_INSTALLED_BYTES = 512L * 1024 * 1024
    const val INSTALL_HEADROOM_BYTES = 16L * 1024 * 1024

    enum class WarmInstallAction {
        LOAD,
        REUSE,
        STAGE_FOR_RESTART,
    }

    fun validateManifest(manifest: PluginManifest): List<String> {
        val errors = mutableListOf<String>()
        if (manifest.id.isBlank()) {
            errors.add("id is required")
        } else if (!isValidPluginId(manifest.id)) {
            errors.add("id contains invalid characters")
        }
        if (manifest.name.isBlank()) errors.add("name is required")
        if (manifest.version.isBlank()) errors.add("version is required")
        if (manifest.runtime != "js" && manifest.entryClass.isBlank()) errors.add("entryClass is required")
        if (manifest.supportedFormats.isEmpty()) errors.add("supportedFormats is required")
        if (manifest.abiVersion > SUPPORTED_ABI_VERSION) {
            errors.add("abiVersion ${manifest.abiVersion} is not supported (max $SUPPORTED_ABI_VERSION)")
        }
        return errors
    }

    fun selectRuntimeAbi(supportedAbis: Array<String>): String {
        return supportedAbis.firstOrNull() ?: "arm64-v8a"
    }

    fun isCompatible(manifest: PluginManifest, runtimeAbi: String, appVersionCode: Int): Boolean {
        return (manifest.abi.isEmpty() || manifest.abi == runtimeAbi) &&
            manifest.minAppVersion <= appVersionCode
    }

    fun isInstallable(manifest: PluginManifest): Boolean {
        return manifest.downloadUrl.isNotBlank() && manifest.checksum.isNotBlank() && manifest.sizeBytes > 0
    }

    fun isValidPluginId(id: String): Boolean = id.matches(Regex("[A-Za-z0-9][A-Za-z0-9._-]{0,127}"))

    fun requireExtractionWithinLimits(entryCount: Int, totalBytes: Long) {
        require(entryCount <= MAX_PLUGIN_ENTRIES) { "Plugin zip contains too many entries" }
        require(totalBytes <= MAX_INSTALLED_BYTES) { "Plugin exceeds its installed size limit" }
    }

    fun requiredInstallSpace(expandedBytes: Long): Long {
        require(expandedBytes >= 0) { "Plugin size must be non-negative" }
        return Math.addExact(expandedBytes, INSTALL_HEADROOM_BYTES)
    }

    fun requireInstallSpace(usableBytes: Long, expandedBytes: Long) {
        val required = requiredInstallSpace(expandedBytes)
        require(usableBytes >= required) {
            "Not enough storage to install plugin: $required bytes required, $usableBytes available"
        }
    }

    fun warmInstallAction(
        warmManifest: PluginManifest?,
        hasNativeInstance: Boolean,
        incoming: PluginManifest,
    ): WarmInstallAction {
        if (warmManifest == null || !hasNativeInstance) return WarmInstallAction.LOAD
        val sameArtifact = warmManifest.version == incoming.version &&
            (incoming.checksum.isEmpty() || warmManifest.checksum.equals(incoming.checksum, ignoreCase = true))
        return if (sameArtifact) WarmInstallAction.REUSE else WarmInstallAction.STAGE_FOR_RESTART
    }

    fun zipEntryDestination(root: File, entryName: String): File {
        val canonicalRoot = root.canonicalFile
        val destination = File(canonicalRoot, entryName).canonicalFile
        val rootPrefix = canonicalRoot.path.trimEnd(File.separatorChar) + File.separator
        if (destination != canonicalRoot && !destination.path.startsWith(rootPrefix)) {
            throw IllegalArgumentException("Plugin zip contains an unsafe path: $entryName")
        }
        return destination
    }

    fun acceptsUnsignedCatalog(debugBuild: Boolean): Boolean = debugBuild
}
