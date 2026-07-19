package ai.viewit.app

import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge
import java.io.File

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    WebView.setWebContentsDebuggingEnabled(true)
    super.onCreate(savedInstanceState)
    consumeIncomingIntent(intent)
  }

  override fun onNewIntent(intent: Intent) {
    super.onNewIntent(intent)
    setIntent(intent)
    consumeIncomingIntent(intent)
  }

  private fun consumeIncomingIntent(intent: Intent?) {
    if (intent == null) return
    val action = intent.action ?: return
    if (
      action != Intent.ACTION_VIEW &&
      action != Intent.ACTION_SEND &&
      action != Intent.ACTION_SEND_MULTIPLE
    ) {
      return
    }

    val uris = mutableListOf<String>()

    @Suppress("DEPRECATION")
    val stream = intent.getParcelableExtra<Uri>(Intent.EXTRA_STREAM)
    if (stream != null) {
      uris.add(grantAndString(stream))
    }

    if (action == Intent.ACTION_SEND_MULTIPLE) {
      @Suppress("DEPRECATION")
      val list = intent.getParcelableArrayListExtra<Uri>(Intent.EXTRA_STREAM)
      list?.forEach { uris.add(grantAndString(it)) }
    }

    intent.data?.let { uris.add(grantAndString(it)) }

    if (uris.isEmpty()) return

    try {
      // Same dir as Tauri `app_data_dir()` (Context.getFilesDir).
      val f = File(applicationContext.filesDir, "viewit_pending_opens.txt")
      f.appendText(uris.joinToString("\n") + "\n")
    } catch (e: Exception) {
      android.util.Log.e("ViewIt", "pending opens write failed", e)
    }
  }

  private fun grantAndString(uri: Uri): String {
    try {
      val flags = Intent.FLAG_GRANT_READ_URI_PERMISSION
      if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.KITKAT) {
        try {
          contentResolver.takePersistableUriPermission(uri, flags)
        } catch (_: SecurityException) {
        }
      }
    } catch (_: Exception) {
    }
    return uri.toString()
  }
}