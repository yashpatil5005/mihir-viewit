package ai.viewit.app

object MediaWorkerProtocol {
    const val MSG_TRANSCODE = 1
    const val MSG_CANCEL = 2
    const val MSG_TEST_DELAY = 3
    const val MSG_PROGRESS = 101
    const val MSG_COMPLETE = 102
    const val MSG_ERROR = 103

    const val KEY_REQUEST_ID = "requestId"
    const val KEY_INPUT_PATH = "inputPath"
    const val KEY_OUTPUT_PATH = "outputPath"
    const val KEY_EXT = "ext"
    const val KEY_PROGRESS = "progress"
    const val KEY_DELAY_MS = "delayMs"
    const val KEY_ERROR = "error"
    const val KEY_PLUGIN_ID = "pluginId"

    const val MAX_PATH_LENGTH = 16_384

    fun validRequest(requestId: String, inputPath: String, outputPath: String, ext: String): Boolean =
        requestId.matches(Regex("[A-Za-z0-9._-]{1,128}")) &&
            inputPath.length in 1..MAX_PATH_LENGTH &&
            outputPath.length in 1..MAX_PATH_LENGTH &&
            ext.matches(Regex("[A-Za-z0-9]{1,16}"))
}
