package ai.viewit.plugins.ffmpeg_transcoder;

import android.content.Context;
import android.net.Uri;
import com.arthenica.ffmpegkit.FFmpegKit;
import com.arthenica.ffmpegkit.FFmpegSession;
import com.arthenica.ffmpegkit.FFprobeKit;
import com.arthenica.ffmpegkit.MediaInformation;
import com.arthenica.ffmpegkit.MediaInformationSession;
import com.arthenica.ffmpegkit.ReturnCode;
import com.arthenica.ffmpegkit.StreamInformation;
import java.io.File;
import java.util.List;

public class Plugin implements ai.viewit.app.ViewItPlugin {
    private Context context;

    @Override
    public String getId() { return "ffmpeg-transcoder"; }

    @Override
    public String getVersion() { return "1.0.2"; }

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

            String inputStr = "\"" + inputPath + "\"";
            String outputQ = "\"" + output.getAbsolutePath() + "\"";
            MediaInformationSession informationSession = FFprobeKit.getMediaInformation(inputPath);
            MediaInformation information = informationSession.getMediaInformation();
            if (information == null) return false;
            double durationMs = parseDurationMs(information.getDuration());
            boolean hasVideo = hasStream(information.getStreams(), "video");
            boolean hasAudio = hasStream(information.getStreams(), "audio");
            if (!hasVideo && !hasAudio) return false;

            String[] cmds = hasVideo ? new String[] {
                "-y -i " + inputStr + " -map 0:v:0 -map 0:a? -c:v h264_mediacodec -b:v 2M -c:a aac -b:a 128k -movflags +faststart " + outputQ,
            } : new String[] {
                "-y -i " + inputStr + " -map 0:a:0 -vn -c:a aac -b:a 128k -movflags +faststart " + outputQ,
            };

            final float[] lastProgress = {0.0f};
            for (String cmd : cmds) {
                updateProgress(onProgress, lastProgress, 0.01f);
                FFmpegSession session = FFmpegKit.executeAsync(
                    cmd,
                    completed -> {},
                    null,
                    statistics -> {
                        if (onProgress != null && durationMs > 0) {
                            updateProgress(
                                onProgress,
                                lastProgress,
                                (float) Math.min(0.99, statistics.getTime() / durationMs)
                            );
                        }
                    }
                );
                while (session.getReturnCode() == null) {
                    Thread.sleep(25);
                }
                if (ReturnCode.isSuccess(session.getReturnCode()) && output.isFile() && output.length() > 0) {
                    updateProgress(onProgress, lastProgress, 1.0f);
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

    private static synchronized void updateProgress(
        ai.viewit.app.PluginProgress callback,
        float[] lastProgress,
        float value
    ) {
        if (callback == null) return;
        float bounded = Math.max(lastProgress[0], Math.min(1.0f, value));
        if (bounded > lastProgress[0] || bounded == 1.0f) {
            lastProgress[0] = bounded;
            callback.update(bounded);
        }
    }

    private static boolean hasStream(List<StreamInformation> streams, String type) {
        if (streams == null) return false;
        for (StreamInformation stream : streams) {
            if (type.equals(stream.getType())) return true;
        }
        return false;
    }

    private static double parseDurationMs(String duration) {
        if (duration == null) return 0;
        try {
            return Double.parseDouble(duration) * 1000.0;
        } catch (NumberFormatException ignored) {
            return 0;
        }
    }

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
