package ai.viewit.app;

import android.content.Context;
import android.net.Uri;
import java.io.File;
import java.util.List;

public interface ViewItPlugin {
    String getId();
    String getVersion();
    List<String> getSupportedFormats();
    void initialize(Context context);
    boolean canHandle(String mimeType);
    boolean canHandleExt(String extension);
    boolean transcode(Uri inputUri, File outputFile, PluginProgress onProgress);
    void cleanup();
}
