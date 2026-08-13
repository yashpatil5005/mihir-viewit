package ai.viewit.app

import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test

class PluginRuntimePolicyTest {
    @Test
    fun `native manifests require an entry class but JavaScript manifests do not`() {
        val nativeErrors = PluginRuntimePolicy.validateManifest(manifest(entryClass = ""))
        val jsErrors = PluginRuntimePolicy.validateManifest(manifest(entryClass = "", runtime = "js"))

        assertTrue(nativeErrors.contains("entryClass is required"))
        assertFalse(jsErrors.contains("entryClass is required"))
    }

    @Test
    fun `manifests reject newer host ABI contracts`() {
        val errors = PluginRuntimePolicy.validateManifest(
            manifest(abiVersion = PluginRuntimePolicy.SUPPORTED_ABI_VERSION + 1),
        )

        assertEquals(listOf("abiVersion 2 is not supported (max 1)"), errors)
    }

    @Test
    fun `manifest IDs cannot address paths outside the plugin directory`() {
        assertEquals(
            listOf("id contains invalid characters"),
            PluginRuntimePolicy.validateManifest(manifest(id = "../outside")),
        )
    }

    @Test
    fun `runtime ABI uses device preference and has a deterministic fallback`() {
        assertEquals("x86_64", PluginRuntimePolicy.selectRuntimeAbi(arrayOf("x86_64", "arm64-v8a")))
        assertEquals("arm64-v8a", PluginRuntimePolicy.selectRuntimeAbi(emptyArray()))
    }

    @Test
    fun `catalog compatibility requires matching ABI and supported app version`() {
        assertTrue(PluginRuntimePolicy.isCompatible(manifest(abi = "arm64-v8a"), "arm64-v8a", 1))
        assertFalse(PluginRuntimePolicy.isCompatible(manifest(abi = "x86_64"), "arm64-v8a", 1))
        assertFalse(PluginRuntimePolicy.isCompatible(manifest(minAppVersion = 2), "arm64-v8a", 1))
    }

    @Test
    fun `catalog packages require complete integrity metadata`() {
        assertTrue(PluginRuntimePolicy.isInstallable(manifest()))
        assertFalse(PluginRuntimePolicy.isInstallable(manifest(checksum = "")))
        assertFalse(PluginRuntimePolicy.isInstallable(manifest(sizeBytes = 0)))
    }

    @Test
    fun `same native artifact is reused after removal`() {
        val installed = manifest(version = "1.2.3", checksum = "ABC")
        val incoming = manifest(version = "1.2.3", checksum = "abc")

        assertEquals(
            PluginRuntimePolicy.WarmInstallAction.REUSE,
            PluginRuntimePolicy.warmInstallAction(installed, true, incoming),
        )
    }

    @Test
    fun `native upgrade is staged for restart while JavaScript reloads`() {
        val installed = manifest(version = "1.0.0")
        val upgrade = manifest(version = "2.0.0")

        assertEquals(
            PluginRuntimePolicy.WarmInstallAction.STAGE_FOR_RESTART,
            PluginRuntimePolicy.warmInstallAction(installed, true, upgrade),
        )
        assertEquals(
            PluginRuntimePolicy.WarmInstallAction.LOAD,
            PluginRuntimePolicy.warmInstallAction(installed, false, upgrade),
        )
    }

    @Test
    fun `zip entries cannot escape the install directory`() {
        val root = File("build/test-plugin-root")

        assertEquals(File(root, "web/index.js").canonicalFile, PluginRuntimePolicy.zipEntryDestination(root, "web/index.js"))
        assertThrows(IllegalArgumentException::class.java) {
            PluginRuntimePolicy.zipEntryDestination(root, "../outside.dex")
        }
    }

    @Test
    fun `plugin extraction enforces entry and expanded size limits`() {
        val sizeLimit = PluginRuntimePolicy.installedSizeLimit(100)

        PluginRuntimePolicy.requireExtractionWithinLimits(1, 100, sizeLimit)
        assertThrows(IllegalArgumentException::class.java) {
            PluginRuntimePolicy.requireExtractionWithinLimits(1, 101, sizeLimit)
        }
        assertThrows(IllegalArgumentException::class.java) {
            PluginRuntimePolicy.requireExtractionWithinLimits(
                PluginRuntimePolicy.MAX_PLUGIN_ENTRIES + 1,
                0,
                sizeLimit,
            )
        }
    }

    @Test
    fun `unsigned catalogs are accepted only by debug builds`() {
        assertTrue(PluginRuntimePolicy.acceptsUnsignedCatalog(true))
        assertFalse(PluginRuntimePolicy.acceptsUnsignedCatalog(false))
    }

    private fun manifest(
        id: String = "test-plugin",
        entryClass: String = "ai.viewit.Plugin",
        runtime: String = "native",
        abi: String = "",
        abiVersion: Int = 1,
        minAppVersion: Int = 1,
        version: String = "1.0.0",
        checksum: String = "abc123",
        sizeBytes: Long = 10,
    ) = PluginManifest(
        id = id,
        name = "Test Plugin",
        version = version,
        description = "",
        minAppVersion = minAppVersion,
        entryClass = entryClass,
        supportedFormats = listOf("txt"),
        downloadUrl = "https://example.test/plugin.zip",
        sizeBytes = sizeBytes,
        installedSizeBytes = sizeBytes,
        checksum = checksum,
        abi = abi,
        abiVersion = abiVersion,
        runtime = runtime,
    )
}
