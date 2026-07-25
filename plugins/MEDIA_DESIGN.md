# Media Runtime and External Services Design

## Overview

This document describes the design for handling unsupported media formats
through external services rather than in-process dynamic-Dex/JNI plugins.

## Problem

FFmpegKit dynamic-Dex/JNI plugin is intentionally not shipped because:
- Native library loading in WebView processes is fragile
- Dynamic Dex loading requires complex classloader management
- Licensing/distribution constraints around FFmpeg binaries
- Battery/storage impact from conversion on mobile

## Target Architecture

### Desktop
- Spawn bundled or user-configured `ffmpeg` executable through scoped command
- Use Tauri shell plugin with allowlist for ffmpeg path
- Sandboxed temp output in app cache directory
- Progress reporting via stderr parsing

### Android
- Prefer platform decoders/native player first (MediaPlayer, ExoPlayer)
- Unsupported formats: use out-of-process bound service or app-extension package
- Service owns native libs at install time (no dynamic loading)
- User consent required for heavy conversions (battery/storage warning)

## Plugin ABI

### `media.inspect` capability
```kotlin
interface MediaInspectPlugin {
    fun inspect(uri: Uri): MediaInfo?
}

data class MediaInfo(
    val mimeType: String,
    val duration: Long?,       // milliseconds
    val width: Int?,           // video only
    val height: Int?,          // video only
    val codec: String?,
    val bitrate: Long?,        // bits per second
    val playable: Boolean,     // can WebView/platform play natively?
    val suggestedAction: String // "play-native" | "convert" | "external"
)
```

### `media.convert` capability
```kotlin
interface MediaConvertPlugin {
    fun convert(input: Uri, outputMime: String, output: File, onProgress: PluginProgress): Boolean
}

interface PluginProgress {
    fun onProgress(percent: Float)
    fun onComplete(output: File)
    fun onError(error: String)
}
```

## Desktop FFmpeg Command Policy

```json
{
  "shell": {
    "allow": [
      { "cmd": "ffmpeg", "args": true },
      { "cmd": "/usr/bin/ffmpeg", "args": true },
      { "cmd": "C:\\ffmpeg\\bin\\ffmpeg.exe", "args": true }
    ]
  }
}
```

Conversion command template:
```
ffmpeg -i {input} -c:v libx264 -preset fast -crf 23 -c:a aac -b:a 128k {output}
```

## Sandboxed Temp Output

- Output written to `getExternalCacheDir()/viewit/transcode/` on Android
- Output written to `app cache dir` on desktop
- Auto-cleanup after 24 hours or on app restart
- User can manually clear from settings

## Progress/Cancel/Error Reporting

- Progress: parsed from ffmpeg stderr `time=` field
- Cancel: kill process group, delete partial output
- Error: surface ffmpeg error message, suggest alternatives

## User Consent

Before conversion:
1. Show estimated output size (input size * 0.8 for video)
2. Show estimated time (input duration * 0.5 for fast preset)
3. Battery warning for files > 5 minutes
4. Storage warning if free space < 2x estimated output

## UI States

- `playable`: render in MediaViewer with `<video>`/`<audio>`
- `inspectable`: show MediaInfo panel with "Convert to MP4" button
- `unsupported`: show "Open with external app" button

## Risks

- Android service/app-extension complexity
- Licensing/distribution constraints around FFmpeg
- Battery/storage impact from conversion
- Process management on low-memory devices

## Status

- `media.inspect` ABI: designed, not implemented
- `media.convert` ABI: designed, not implemented
- Desktop FFmpeg policy: designed, not implemented
- Android external service: designed, not implemented
- UI for conversion progress: designed, not implemented

## Next Steps

1. Implement `media.inspect` for Android MediaPlayer probing
2. Implement desktop FFmpeg spawn via Tauri shell
3. Add conversion progress UI to MediaViewer
4. Add user consent dialog for heavy conversions
