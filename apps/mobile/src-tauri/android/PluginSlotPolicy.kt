package ai.viewit.app

import java.io.File
import java.security.MessageDigest

data class PluginSlotId(
    val version: String,
    val artifact: String,
) {
    val directoryName: String = "${safe(version)}-$artifact"

    companion object {
        fun from(manifest: PluginManifest): PluginSlotId {
            val artifact = manifest.checksum.lowercase().take(16).ifEmpty {
                MessageDigest.getInstance("SHA-256")
                    .digest("${manifest.id}:${manifest.version}".toByteArray())
                    .take(8)
                    .joinToString("") { "%02x".format(it) }
            }
            return PluginSlotId(manifest.version, artifact)
        }

        private fun safe(value: String): String = value.replace(Regex("[^A-Za-z0-9._-]"), "_")
    }
}

data class PluginSlotState(
    val active: String? = null,
    val previous: String? = null,
    val pending: String? = null,
)

object PluginSlotPolicy {
    fun pluginRoot(pluginsDir: File, pluginId: String): File = File(pluginsDir, pluginId)
    fun versionsDir(pluginRoot: File): File = File(pluginRoot, "versions")
    fun slotDir(pluginRoot: File, slot: PluginSlotId): File = File(versionsDir(pluginRoot), slot.directoryName)
    fun stateFile(pluginRoot: File): File = File(pluginRoot, "state.json")

    fun activate(state: PluginSlotState, slot: String): PluginSlotState = PluginSlotState(
        active = slot,
        previous = state.active?.takeIf { it != slot } ?: state.previous,
        pending = null,
    )

    fun stage(state: PluginSlotState, slot: String): PluginSlotState = state.copy(pending = slot)

    fun rollback(state: PluginSlotState): PluginSlotState? {
        val previous = state.previous ?: return null
        return PluginSlotState(active = previous, previous = state.active, pending = null)
    }
}
