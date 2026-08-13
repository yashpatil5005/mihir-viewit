package ai.viewit.app

import org.junit.Assert.assertEquals
import org.junit.Test

class AndroidCallbackScriptsTest {
    @Test
    fun `document callback supports current and legacy bridge names`() {
        assertEquals(
            "(window._documentPluginCallback || window._docPluginCallback) && " +
                "(window._documentPluginCallback || window._docPluginCallback)({\"id\":\"callback-1\"})",
            AndroidCallbackScripts.documentPlugin("{\"id\":\"callback-1\"}"),
        )
    }
}
