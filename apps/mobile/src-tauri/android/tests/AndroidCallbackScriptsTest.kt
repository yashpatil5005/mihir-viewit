package ai.viewit.app

import org.junit.Assert.assertEquals
import org.junit.Test

class AndroidCallbackScriptsTest {
    @Test
    fun `document callback dispatches through standard bridge name`() {
        assertEquals(
            "window._documentPluginCallback && window._documentPluginCallback({\"id\":\"callback-1\"})",
            AndroidCallbackScripts.documentPlugin("{\"id\":\"callback-1\"}"),
        )
    }
}
