package ai.viewit.app

import android.content.Intent
import android.database.Cursor
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.provider.MediaStore
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

    val lines = mutableListOf<String>()

    @Suppress("DEPRECATION")
    val stream = intent.getParcelableExtra<Uri>(Intent.EXTRA_STREAM)
    if (stream != null) {
      lines.add(grantWithDisplayName(stream))
    }

    if (action == Intent.ACTION_SEND_MULTIPLE) {
      @Suppress("DEPRECATION")
      val list = intent.getParcelableArrayListExtra<Uri>(Intent.EXTRA_STREAM)
      list?.forEach { lines.add(grantWithDisplayName(it)) }
    }

    intent.data?.let { lines.add(grantWithDisplayName(it)) }

    if (lines.isEmpty()) return

    try {
      val f = File(applicationContext.filesDir, "viewit_pending_opens.txt")
      f.writeText(lines.joinToString("\n") + "\n")
    } catch (e: Exception) {
      android.util.Log.e("ViewIt", "pending opens write failed", e)
    }
  }

  private fun grantWithDisplayName(uri: Uri): String {
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
    // Resolve the display name from MediaStore
    val displayName = queryDisplayName(uri)
    return if (displayName != null) {
      "${uri}\t$displayName"
    } else {
      uri.toString()
    }
  }

  private fun queryDisplayName(uri: Uri): String? {
    val projection = arrayOf(MediaStore.MediaColumns.DISPLAY_NAME)
    val cursor: Cursor? = contentResolver.query(uri, projection, null, null, null)
    return cursor?.use {
      if (it.moveToFirst()) {
        val idx = it.getColumnIndex(MediaStore.MediaColumns.DISPLAY_NAME)
        if (idx >= 0) it.getString(idx) else null
      } else null
    }
  }
}