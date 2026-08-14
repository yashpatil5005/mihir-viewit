package ai.viewit.app

object AndroidCallbackScripts {
    fun documentPlugin(payloadJson: String): String {
        return "window._documentPluginCallback && window._documentPluginCallback($payloadJson)"
    }
}
