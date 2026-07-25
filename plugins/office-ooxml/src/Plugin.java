package ai.viewit.plugins.office_ooxml;

import android.content.Context;
import android.net.Uri;
import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileInputStream;
import java.io.InputStream;
import org.json.JSONObject;

public class Plugin implements ai.viewit.app.ViewItDocumentPlugin {
    private Context context;
    private static boolean nativeLoaded = false;

    @Override
    public String getId() { return "office-ooxml"; }

    @Override
    public String getVersion() { return "0.3.0"; }

    @Override
    public java.util.List<String> getSupportedFormats() {
        return java.util.Arrays.asList("docx", "xlsx", "xls", "pptx", "docm", "xlsm", "pptm");
    }

    @Override
    public void initialize(Context context) {
        this.context = context.getApplicationContext();
        if (nativeLoaded) return;
        try {
            System.loadLibrary("viewit_plugin_office_ooxml");
            nativeLoaded = true;
            return;
        } catch (UnsatisfiedLinkError ignored) {
            // Fall back to explicit paths below for devices that do not resolve
            // plugin-native libraries through the DexClassLoader search path.
        }

        java.util.List<File> libRoots = java.util.Arrays.asList(
            new File(this.context.getFilesDir(), "plugins/office-ooxml.staging/lib"),
            new File(this.context.getFilesDir(), "plugins/office-ooxml/lib")
        );
        java.util.List<String> candidates = new java.util.ArrayList<>();
        String arch = System.getProperty("os.arch", "").toLowerCase();
        if (arch.equals("aarch64") || arch.equals("arm64")) {
            candidates.add("arm64-v8a");
        } else if (arch.equals("x86_64") || arch.equals("amd64")) {
            candidates.add("x86_64");
        }
        for (String abi : android.os.Build.SUPPORTED_ABIS) candidates.add(abi);

        UnsatisfiedLinkError lastError = null;
        for (File libRoot : libRoots) {
            for (String abi : candidates) {
                File lib = new File(libRoot, abi + "/libviewit_plugin_office_ooxml.so");
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
        throw new UnsatisfiedLinkError("No compatible Office OOXML native library found in plugin lib directories");
    }

    @Override
    public boolean canHandle(String mimeType) {
        if (mimeType == null) return false;
        return mimeType.contains("wordprocessingml")
            || mimeType.contains("spreadsheetml")
            || mimeType.contains("presentationml");
    }

    @Override
    public boolean canHandleExt(String extension) {
        if (extension == null) return false;
        switch (extension.toLowerCase()) {
            case "docx":
            case "xlsx":
            case "xls":
            case "pptx":
            case "docm":
            case "xlsm":
            case "pptm":
                return true;
            default:
                return false;
        }
    }

    @Override
    public String render(Uri inputUri, String ext) {
        String cleanExt = ext == null ? "" : ext.toLowerCase();
        try {
            byte[] bytes = readAll(inputUri);
            String rendered = renderNative(bytes, cleanExt);
            JSONObject result = new JSONObject(rendered);
            if (result.has("error")) throw new IllegalArgumentException(result.getString("error"));
            return rendered;
        } catch (Exception e) {
            throw new RuntimeException(e.getMessage(), e);
        }
    }

    @Override
    public void cleanup() {}

    private static native String renderNative(byte[] bytes, String ext);

    private byte[] readAll(Uri uri) throws Exception {
        InputStream input = context.getContentResolver().openInputStream(uri);
        if (input == null && "file".equals(uri.getScheme()) && uri.getPath() != null) {
            input = new FileInputStream(new File(uri.getPath()));
        }
        if (input == null) throw new IllegalArgumentException("Cannot open input URI");
        try (InputStream in = input; ByteArrayOutputStream out = new ByteArrayOutputStream()) {
            byte[] buffer = new byte[8192];
            int n;
            while ((n = in.read(buffer)) >= 0) out.write(buffer, 0, n);
            return out.toByteArray();
        }
    }

}
