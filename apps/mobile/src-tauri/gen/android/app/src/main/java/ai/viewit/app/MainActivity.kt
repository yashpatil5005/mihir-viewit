package ai.viewit.app

import android.os.Bundle
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    WebView.setWebContentsDebuggingEnabled(true)
    super.onCreate(savedInstanceState)
  }

  companion object {
    init {
      // Pre-load bundled libpdfium.so (per ADR 0002) so Rust's dlopen("libpdfium.so")
      // via pdfium-render's bind_to_system_library() succeeds on Android API 24+.
      runCatching { System.loadLibrary("pdfium") }
    }
  }
}
