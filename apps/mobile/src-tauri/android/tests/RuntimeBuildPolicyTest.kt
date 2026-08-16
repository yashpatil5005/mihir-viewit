package ai.viewit.app

import org.junit.Assert.assertFalse
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test

class RuntimeBuildPolicyTest {
    @Test
    fun `development enables local diagnostics and unsigned catalogs`() {
        assertTrue(RuntimeBuildPolicy.enablesWebViewDebugging(RuntimeBuildPolicy.DEVELOPMENT))
        assertTrue(RuntimeBuildPolicy.enablesLocalPluginInstall(RuntimeBuildPolicy.DEVELOPMENT))
        assertTrue(RuntimeBuildPolicy.enablesDebugIntents(RuntimeBuildPolicy.DEVELOPMENT))
        assertTrue(RuntimeBuildPolicy.acceptsUnsignedCatalog(RuntimeBuildPolicy.DEVELOPMENT))
    }

    @Test
    fun `device test keeps diagnostics but requires signed catalogs`() {
        assertTrue(RuntimeBuildPolicy.enablesWebViewDebugging(RuntimeBuildPolicy.DEVICE_TEST))
        assertTrue(RuntimeBuildPolicy.enablesLocalPluginInstall(RuntimeBuildPolicy.DEVICE_TEST))
        assertTrue(RuntimeBuildPolicy.enablesDebugIntents(RuntimeBuildPolicy.DEVICE_TEST))
        assertFalse(RuntimeBuildPolicy.acceptsUnsignedCatalog(RuntimeBuildPolicy.DEVICE_TEST))
    }

    @Test
    fun `production disables local and debugging surfaces`() {
        assertFalse(RuntimeBuildPolicy.enablesWebViewDebugging(RuntimeBuildPolicy.PRODUCTION))
        assertFalse(RuntimeBuildPolicy.enablesLocalPluginInstall(RuntimeBuildPolicy.PRODUCTION))
        assertFalse(RuntimeBuildPolicy.enablesDebugIntents(RuntimeBuildPolicy.PRODUCTION))
        assertFalse(RuntimeBuildPolicy.acceptsUnsignedCatalog(RuntimeBuildPolicy.PRODUCTION))
    }

    @Test
    fun `unknown profiles fail closed`() {
        assertThrows(IllegalArgumentException::class.java) {
            RuntimeBuildPolicy.enablesWebViewDebugging("release")
        }
    }
}
