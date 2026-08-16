package ai.viewit.app

object RuntimeBuildPolicy {
    const val DEVELOPMENT = "development"
    const val DEVICE_TEST = "device-test"
    const val PRODUCTION = "production"

    private val supported = setOf(DEVELOPMENT, DEVICE_TEST, PRODUCTION)

    fun requireValid(profile: String): String {
        require(profile in supported) { "Unsupported ViewIt build profile: $profile" }
        return profile
    }

    fun enablesWebViewDebugging(profile: String): Boolean = requireValid(profile) != PRODUCTION

    fun enablesLocalPluginInstall(profile: String): Boolean = requireValid(profile) != PRODUCTION

    fun enablesDebugIntents(profile: String): Boolean = requireValid(profile) != PRODUCTION

    fun acceptsUnsignedCatalog(profile: String): Boolean = requireValid(profile) == DEVELOPMENT
}
