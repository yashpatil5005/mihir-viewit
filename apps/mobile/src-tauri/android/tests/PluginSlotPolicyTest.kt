package ai.viewit.app

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Test

class PluginSlotPolicyTest {
    @Test
    fun `slot identity includes version and artifact`() {
        val first = PluginSlotId.from(manifest(version = "1.0.0", checksum = "a".repeat(64)))
        val second = PluginSlotId.from(manifest(version = "1.0.0", checksum = "b".repeat(64)))

        assertEquals("1.0.0-${"a".repeat(16)}", first.directoryName)
        assertNotEquals(first, second)
    }

    @Test
    fun `activation preserves last known good and clears pending`() {
        val next = PluginSlotPolicy.activate(
            PluginSlotState(active = "1.0.0-aaa", pending = "2.0.0-bbb"),
            "2.0.0-bbb",
        )

        assertEquals("2.0.0-bbb", next.active)
        assertEquals("1.0.0-aaa", next.previous)
        assertNull(next.pending)
    }

    @Test
    fun `rollback swaps active and previous`() {
        assertEquals(
            PluginSlotState(active = "1.0.0-aaa", previous = "2.0.0-bbb"),
            PluginSlotPolicy.rollback(PluginSlotState(active = "2.0.0-bbb", previous = "1.0.0-aaa")),
        )
    }

    private fun manifest(version: String, checksum: String) = PluginManifest(
        id = "test-plugin",
        name = "Test",
        version = version,
        description = "",
        minAppVersion = 1,
        entryClass = "ai.viewit.Test",
        supportedFormats = listOf("txt"),
        downloadUrl = "https://example.test/plugin.zip",
        sizeBytes = 1,
        installedSizeBytes = 1,
        checksum = checksum,
        abi = "arm64-v8a",
    )
}
