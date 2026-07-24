package ai.viewit.app

import android.app.Application

class ViewItApp : Application() {
    lateinit var pluginManager: PluginManager
        private set

    override fun onCreate() {
        super.onCreate()
        pluginManager = PluginManager(this)
        pluginManager.init()
    }
}
