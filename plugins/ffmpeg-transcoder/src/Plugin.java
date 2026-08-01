package ai.viewit.plugins.ffmpeg_transcoder;

import android.content.Context;
import android.net.Uri;
import java.io.File;

public class Plugin implements ai.viewit.app.ViewItPlugin {
    private Context context;

    @Override
    public String getId() { return "ffmpeg-transcoder"; }

    @Override
    public String getVersion() { return "1.0.0"; }

    @Override
    public java.util.List<String> getSupportedFormats() {
        return java.util.Arrays.asList(
            "avi", "mkv", "mov", "webm", "3gp",
            "flv", "asf", "wmv", "wma", "swf", "mxf",
            "mpg", "mpeg", "ts", "m2ts", "mts"
        );
    }

    @Override
    public void initialize(Context ctx) {
        this.context = ctx;
    }

    @Override
    public boolean canHandle(String mimeType) {
        if (mimeType == null) return false;
        switch (mimeType) {
            case "video/x-flv":
            case "video/x-msvideo":
            case "video/x-matroska":
            case "video/quicktime":
            case "video/webm":
            case "video/3gpp":
            case "video/x-ms-wmv":
            case "video/x-ms-asf":
            case "video/x-swf":
            case "video/x-mxf":
            case "video/mpeg":
            case "video/mp2t":
            case "audio/x-ms-wma":
                return true;
            default:
                return false;
        }
    }

    @Override
    public boolean canHandleExt(String ext) {
        if (ext == null) return false;
        switch (ext.toLowerCase()) {
            case "flv":
            case "avi":
            case "mkv":
            case "mov":
            case "webm":
            case "3gp":
            case "asf":
            case "wmv":
            case "wma":
            case "swf":
            case "mxf":
            case "mpg":
            case "mpeg":
            case "ts":
            case "m2ts":
            case "mts":
                return true;
            default:
                return false;
        }
    }

    @Override
    public boolean transcode(Uri input, File output, ai.viewit.app.PluginProgress onProgress) {
        try {
            String inputPath = resolveInputPath(input);
            if (inputPath == null) return false;

            String outputStr = output.getAbsolutePath();
            String inputStr = "\"" + inputPath + "\"";
            String outputQ = "\"" + outputStr + "\"";

            String[] cmds = {
                "-y -i " + inputStr + " -c copy -movflags +faststart " + outputQ,
                "-y -i " + inputStr + " -c:v h264_mediacodec -b:v 2M -c:a aac -b:a 128k " + outputQ,
                "-y -i " + inputStr + " -c:v mpeg4 -q:v 5 -c:a aac -b:a 128k " + outputQ,
            };

            for (String cmd : cmds) {
                com.arthenica.ffmpegkit.FFmpegKit.execute(cmd);
                if (output.exists() && output.length() > 0) {
                    if (onProgress != null) onProgress.update(1.0f);
                    return true;
                }
                output.delete();
            }
            return false;
        } catch (Exception e) {
            android.util.Log.e("FFmpegPlugin", "Transcode failed", e);
            return false;
        }
    }

    @Override
    public void cleanup() {}

    private String resolveInputPath(Uri uri) {
        String scheme = uri.getScheme();
        if ("file".equals(scheme)) return uri.getPath();
        if ("http".equals(scheme) || "https".equals(scheme)) return uri.toString();
        if ("content".equals(scheme)) {
            try {
                android.database.Cursor cursor = context.getContentResolver().query(
                    uri, null, null, null, null);
                if (cursor != null) {
                    try {
                        if (cursor.moveToFirst()) {
                            int idx = cursor.getColumnIndex("_data");
                            if (idx >= 0) {
                                String path = cursor.getString(idx);
                                if (path != null && new File(path).exists()) return path;
                            }
                        }
                    } finally { cursor.close(); }
                }
            } catch (Exception ignored) {}
        }
        return null;
    }
}
