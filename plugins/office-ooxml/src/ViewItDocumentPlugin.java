package ai.viewit.app;

import android.content.Context;
import android.net.Uri;
import java.util.List;

public interface ViewItDocumentPlugin {
    String getId();
    String getVersion();
    List<String> getSupportedFormats();
    void initialize(Context context);
    boolean canHandle(String mimeType);
    boolean canHandleExt(String extension);
    String render(Uri inputUri, String ext);
    void cleanup();
}
