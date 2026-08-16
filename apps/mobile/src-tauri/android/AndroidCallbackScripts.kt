package ai.viewit.app

object AndroidCallbackScripts {
    fun documentPlugin(payloadJson: String): String {
        return "window.__viewitBridgeDispatch ? " +
            "window.__viewitBridgeDispatch(Object.assign({event: ($payloadJson).error ? 'error' : 'complete', result: ($payloadJson).document}, $payloadJson)) : " +
            "(window.__viewitBridgeQueue = window.__viewitBridgeQueue || []).push(Object.assign({event: ($payloadJson).error ? 'error' : 'complete', result: ($payloadJson).document}, $payloadJson));" +
            "window._documentPluginCallback && window._documentPluginCallback($payloadJson)"
    }
}
