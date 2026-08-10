package ai.viewit.plugins.officeuniversal;

import android.content.Context;
import android.net.Uri;
import ai.viewit.app.ViewItDocumentPlugin;
import java.io.File;
import java.io.FileInputStream;
import java.io.InputStream;
import org.json.JSONObject;

/**
 * Downloadable office-universal plugin (fmt-office + bundled pptx-vanilla).
 * Native lib is loaded tolerantly from the plugin's own lib dir (the plugin
 * loader loads it too); it MUST NOT rely on System.loadLibrary-by-name because
 * nothing is baked into the APK.
 */
public class OfficeUniversalPlugin implements ViewItDocumentPlugin {
    private Context context;
    private static boolean nativeLoaded = false;

    @Override
    public String getId() { return "office-universal"; }

    @Override
    public String getVersion() { return "0.1.0"; }

    @Override
    public java.util.List<String> getSupportedFormats() {
        return java.util.Arrays.asList(
            "docx", "docm", "dotx", "dotm",
            "xlsx", "xlsm", "xlsb", "xls",
            "pptx", "pptm", "potx",
            "odt", "ott", "ods", "ots", "odp", "otp",
            "doc", "ppt");
    }

    @Override
    public void initialize(Context context) {
        this.context = context.getApplicationContext();
        if (nativeLoaded) return;
        try {
            System.loadLibrary("viewit_plugin_office_universal");
            nativeLoaded = true;
            return;
        } catch (UnsatisfiedLinkError ignored) {
            // fall through to explicit path below
        }
        java.util.List<File> libRoots = java.util.Arrays.asList(
            new File(this.context.getFilesDir(), "plugins/office-universal.staging/lib"),
            new File(this.context.getFilesDir(), "plugins/office-universal/lib"),
            new File(this.context.getFilesDir(), "plugins/office-universal")
        );
        java.util.LinkedHashMap<String, File> candidates = new java.util.LinkedHashMap<>();
        String arch = System.getProperty("os.arch", "").toLowerCase();
        if (arch.equals("aarch64") || arch.equals("arm64")) candidates.put("arm64-v8a", null);
        else if (arch.equals("x86_64") || arch.equals("amd64")) candidates.put("x86_64", null);
        for (String abi : android.os.Build.SUPPORTED_ABIS) candidates.put(abi, null);
        UnsatisfiedLinkError lastError = null;
        for (File libRoot : libRoots) {
            for (String abi : candidates.keySet()) {
                File lib = new File(libRoot, abi + "/libviewit_plugin_office_universal.so");
                if (!lib.exists()) continue;
                try {
                    System.load(lib.getAbsolutePath());
                    nativeLoaded = true;
                    return;
                } catch (UnsatisfiedLinkError e) {
                    lastError = e;
                }
            }
        }
        if (lastError != null) throw lastError;
        throw new UnsatisfiedLinkError("No compatible office-universal native library found");
    }

    @Override
    public boolean canHandle(String mimeType) {
        if (mimeType == null) return false;
        return mimeType.contains("wordprocessingml")
            || mimeType.contains("spreadsheetml")
            || mimeType.contains("presentationml")
            || mimeType.contains("oasis.opendocument")
            || mimeType.contains("msword")
            || mimeType.contains("ms-excel")
            || mimeType.contains("ms-powerpoint");
    }

    @Override
    public boolean canHandleExt(String ext) {
        if (ext == null) return false;
        return getSupportedFormats().contains(ext.toLowerCase());
    }

    @Override
    public String render(Uri inputUri, String ext) {
        String cleanExt = ext == null ? "" : ext.toLowerCase();
        try {
            byte[] bytes = readAll(inputUri);
            String rendered = nativeRenderBytes(bytes, cleanExt);
            JSONObject result = new JSONObject(rendered);
            if (result.has("error")) throw new IllegalArgumentException(result.getString("error"));
            return rendered;
        } catch (Exception e) {
            String reason = (e.getMessage() == null ? e.getClass().getSimpleName() : e.getMessage())
                .replace("\\", "\\\\").replace("\"", "\\\"");
            return "{\"kind\":\"unsupported\",\"format\":\"" + cleanExt
                + "\",\"reason\":\"" + reason + "\",\"suggestion\":\"none\"}";
        }
    }

    @Override
    public void cleanup() {}

    private static native boolean nativeCanHandleMimeType(String mimeType);
    private static native boolean nativeCanHandleExt(String ext);
    private static native String nativeRender(String filePath, String ext);
    private static native String nativeRenderBytes(byte[] bytes, String ext);
    private static native void nativeCleanup();

    private byte[] readAll(Uri uri) throws Exception {
        InputStream input = context.getContentResolver().openInputStream(uri);
        if (input == null && "file".equals(uri.getScheme()) && uri.getPath() != null) {
            input = new FileInputStream(new File(uri.getPath()));
        }
        if (input == null) throw new IllegalArgumentException("Cannot open input URI");
        try (InputStream in = input; java.io.ByteArrayOutputStream out = new java.io.ByteArrayOutputStream()) {
            byte[] buffer = new byte[8192];
            int n;
            while ((n = in.read(buffer)) >= 0) out.write(buffer, 0, n);
            return out.toByteArray();
        }
    }
}
