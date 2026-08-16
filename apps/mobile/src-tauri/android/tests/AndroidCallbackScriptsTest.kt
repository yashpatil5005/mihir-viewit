package ai.viewit.app

import org.junit.Assert.assertTrue
import org.junit.Test

class AndroidCallbackScriptsTest {
    @Test
    fun `document callback dispatches through standard bridge name`() {
        val script = AndroidCallbackScripts.documentPlugin("{\"id\":\"callback-1\"}")
        assertTrue(script.contains("window.__viewitBridgeDispatch"))
        assertTrue(script.contains("window.__viewitBridgeQueue"))
        assertTrue(script.contains("window._documentPluginCallback"))
    }
}
