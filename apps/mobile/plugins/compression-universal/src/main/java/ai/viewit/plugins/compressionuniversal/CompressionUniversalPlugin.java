package ai.viewit.plugins.compressionuniversal;

import android.content.Context;
import android.net.Uri;
import ai.viewit.app.ArchivePlugin;
import ai.viewit.app.ViewItDocumentPlugin;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.security.MessageDigest;

/** Downloadable compression-universal plugin (fmt-archive bridge). */
public class CompressionUniversalPlugin implements ViewItDocumentPlugin, ArchivePlugin {
    private Context context;
    private static boolean nativeLoaded = false;

    @Override
    public String getId() { return "compression-universal"; }

    @Override
    public String getVersion() { return "0.1.0"; }

    @Override
    public java.util.List<String> getSupportedFormats() {
        return java.util.Arrays.asList(
            "zip", "7z", "rar", "tar",
            "gz", "tgz", "bz2", "tbz2", "xz", "txz", "zst", "tzst",
            "lz4", "lzma", "tlz",
            "tar.gz", "tar.bz2", "tar.xz", "tar.zst", "tar.lz4", "tar.lzma");
    }

    @Override
    public void initialize(Context context) {
        this.context = context.getApplicationContext();
        if (nativeLoaded) return;
        try {
            System.loadLibrary("viewit_plugin_compression_universal");
            nativeLoaded = true;
            return;
        } catch (UnsatisfiedLinkError ignored) {
            // fall through to explicit path below
        }
        java.util.List<File> libRoots = java.util.Arrays.asList(
            new File(this.context.getFilesDir(), "plugins/compression-universal.staging/lib"),
            new File(this.context.getFilesDir(), "plugins/compression-universal/lib"),
            new File(this.context.getFilesDir(), "plugins/compression-universal")
        );
        java.util.LinkedHashMap<String, File> candidates = new java.util.LinkedHashMap<>();
        String arch = System.getProperty("os.arch", "").toLowerCase();
        if (arch.equals("aarch64") || arch.equals("arm64")) candidates.put("arm64-v8a", null);
        else if (arch.equals("x86_64") || arch.equals("amd64")) candidates.put("x86_64", null);
        for (String abi : android.os.Build.SUPPORTED_ABIS) candidates.put(abi, null);
        UnsatisfiedLinkError lastError = null;
        for (File libRoot : libRoots) {
            for (String abi : candidates.keySet()) {
                File lib = new File(libRoot, abi + "/libviewit_plugin_compression_universal.so");
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
        throw new UnsatisfiedLinkError("No compatible compression-universal native library found");
    }

    @Override
    public boolean canHandle(String mimeType) {
        if (mimeType == null) return false;
        return mimeType.contains("zip") || mimeType.contains("x-7z-compressed")
            || mimeType.contains("x-rar") || mimeType.contains("x-tar")
            || mimeType.contains("gzip") || mimeType.contains("x-bzip2")
            || mimeType.contains("x-xz") || mimeType.contains("zstd");
    }

    @Override
    public boolean canHandleExt(String ext) {
        if (ext == null) return false;
        return getSupportedFormats().contains(ext.toLowerCase());
    }

    @Override
    public String render(Uri inputUri, String ext) {
        String cleanExt = ext == null ? "" : ext.toLowerCase();
        return "{\"kind\":\"unsupported\",\"format\":\"" + cleanExt
            + "\",\"reason\":\"compression-universal renders via the archive bridge\",\"suggestion\":\"none\"}";
    }

    @Override
    public void cleanup() {}

    @Override
    public String listArchive(Uri uri, String name) {
        File file = materialize(uri);
        if (file == null) return "{\"ok\":false,\"error\":\"Unable to read file from URI\"}";
        return nativeListArchive(file.getAbsolutePath(), name);
    }

    @Override
    public byte[] extractEntry(Uri uri, String name, String entryName) {
        File file = materialize(uri);
        if (file == null) return null;
        return nativeExtractEntry(file.getAbsolutePath(), name, entryName);
    }

    @Override
    public String extractEntryToFile(Uri uri, String name, String entryName, File outFile) {
        File file = materialize(uri);
        if (file == null) return "{\"ok\":false,\"error\":\"Unable to read file from URI\"}";
        return nativeExtractEntryToFile(file.getAbsolutePath(), name, entryName, outFile.getAbsolutePath());
    }

    @Override
    public String extractAll(Uri uri, String name, File destDir) {
        File file = materialize(uri);
        if (file == null) return "{\"ok\":false,\"error\":\"Unable to read file from URI\"}";
        return nativeExtractAll(file.getAbsolutePath(), name, destDir.getAbsolutePath());
    }

    @Override
    public String detectFormat(Uri uri, String name) {
        File file = materialize(uri);
        if (file == null) return "unknown";
        return nativeDetectFormat(file.getAbsolutePath());
    }

    /** content:// → cached file; file:// → path as-is. */
    private File materialize(Uri uri) {
        if (uri == null) return null;
        try {
            if ("file".equals(uri.getScheme())) {
                return uri.getPath() == null ? null : new File(uri.getPath());
            }
            MessageDigest md = MessageDigest.getInstance("SHA-256");
            byte[] digest = md.digest(uri.toString().getBytes("UTF-8"));
            StringBuilder sb = new StringBuilder();
            for (int i = 0; i < 12; i++) sb.append(String.format("%02x", digest[i]));
            File dir = new File(context.getCacheDir(), "viewit_compression");
            if (!dir.exists() && !dir.mkdirs()) return null;
            File dest = new File(dir, sb.toString() + ".bin");
            if (!dest.exists() || dest.length() == 0L) {
                InputStream input = context.getContentResolver().openInputStream(uri);
                if (input == null) return null;
                try (InputStream in = input; FileOutputStream out = new FileOutputStream(dest)) {
                    byte[] buffer = new byte[8192];
                    int n;
                    while ((n = in.read(buffer)) >= 0) out.write(buffer, 0, n);
                }
            }
            return dest;
        } catch (Exception e) {
            return null;
        }
    }

    private static native boolean nativeCanHandleMimeType(String mimeType);
    private static native boolean nativeCanHandleExt(String ext);
    private static native String nativeListArchive(String path, String name);
    private static native String nativeDetectFormat(String path);
    private static native byte[] nativeExtractEntry(String path, String name, String entryName);
    private static native String nativeExtractEntryToFile(String path, String name, String entryName, String outPath);
    private static native String nativeExtractAll(String path, String name, String destDir);
    private static native void nativeCleanup();
}
