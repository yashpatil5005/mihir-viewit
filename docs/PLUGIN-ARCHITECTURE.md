# ViewIt Plugin Architecture

ViewIt's moat is the smallest possible base app that can still view common daily files. Plugins must extend format coverage without turning the base APK into a universal codec/office suite bundle.

## Goals

- Keep the Android base APK under the published size gate.
- Let users choose optional runtimes per format when a file is opened.
- Support view plugins first, then write/export plugins with explicit permissions.
- Make plugin download size and installed size visible before install.
- Prefer small, format-specific plugins over broad runtime bundles.
- Avoid dynamic-native designs that cannot be reliably loaded after install on Android.

## Runtime Selection

When a file is opened, the resolver produces:

- format id, such as `xlsx`, `docx`, `flv`, `asf`
- MIME type
- file name and extension
- available built-in runtime
- installed plugin runtimes
- downloadable plugin runtimes from catalog

If more than one runtime can handle the file, ViewIt shows a runtime chooser:

- Built-in viewer, when available
- Installed plugin viewer
- Downloadable plugin viewer, with compressed and installed size
- External app fallback

The user's selection can be remembered per format, but must be reversible in settings.

## Plugin Manifest

Each plugin should declare:

```json
{
  "id": "office-xlsx-reader",
  "name": "Excel Reader",
  "version": "1.0.0",
  "capabilities": ["view"],
  "formats": ["xlsx", "xlsm"],
  "mimeTypes": ["application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"],
  "entry": {
    "android": { "type": "dex", "class": "ai.viewit.plugins.xlsx.Plugin" },
    "web": { "type": "esm", "module": "index.js" }
  },
  "sizeBytes": 1800000,
  "installedSizeBytes": 4200000,
  "permissions": ["read-file"],
  "minAppVersion": 1
}
```

Write-capable plugins add explicit capabilities such as `write`, `export-pdf`, or `modify-in-place`; they must be prompted separately from view-only plugins.

## Android Execution Model

Android plugins should use one of these models:

- Pure JVM/Dex plugin implementing ViewIt's stable app-owned interfaces.
- JS/HTML/CSS plugin rendered in WebView and backed by a small host bridge.
- Host-provided native service, where the base app owns the JNI/native binding and the plugin supplies configuration/assets/codecs that the host can safely load.
- Out-of-process executable worker only where Android platform constraints allow it.

Avoid arbitrary JNI AARs inside dynamically loaded plugins. FFmpegKit showed why: Java classes can be loaded from `DexClassLoader`, but its native `JNI_OnLoad`/class lookup path still resolves through boot/system classloaders in this setup, causing runtime `NoClassDefFoundError`/`FindClass` failures even after dependency classes are present. The current player catches that linkage failure and keeps the app alive, but FFmpegKit is not a viable post-install plugin runtime in this form.

## Stable Plugin ABI

The plugin ABI must use ViewIt-owned types only. Do not expose desugared JDK interfaces such as `java.util.function.Consumer` in plugin method signatures, because R8/desugaring can rewrite app-side signatures in release builds.

Current safe Android ABI shape:

```kotlin
interface ViewItPlugin {
    val id: String
    val version: String
    val supportedFormats: List<String>
    fun initialize(context: Context)
    fun canHandle(mimeType: String): Boolean
    fun canHandleExt(ext: String): Boolean
    fun transcode(input: Uri, output: File, onProgress: PluginProgress): Boolean
    fun cleanup()
}

interface PluginProgress {
    fun update(progress: Float)
}
```

R8 keep rules must preserve every app-owned plugin ABI name and member.

## Size Policy

Every plugin card must show:

- compressed download size
- installed size after unzip
- ABI coverage, such as `arm64-v8a only` or `universal`
- optional dependency size

The Office OOXML runtime is published as ABI-specific ZIPs so users do not download native code their process cannot load:

- `office-ooxml-0.1.0-arm64-v8a.zip`: about 9.0 MB compressed / 25.2 MB installed
- `office-ooxml-0.1.0-x86_64.zip`: about 9.8 MB compressed / 28.7 MB installed

The catalog may contain multiple entries with the same plugin id and different `abi` values. Android filters catalog entries to the current process ABI before presenting runtimes. This matters on emulators because the device can advertise `x86_64` while an arm64-only APK runs translated as an `aarch64` process.

Plugin catalog generation should publish per-ABI artifacts instead of universal ZIPs:

- `ffmpeg-transcoder-android-arm64-v8a.zip`
- `ffmpeg-transcoder-android-armeabi-v7a.zip`
- `ffmpeg-transcoder-android-x86_64.zip`

## Office Plugins

Office enhancement is a plugin concern, not a base-app concern. The base app should keep its lightweight DOCX/XLSX/PPTX previews and stay comfortably under the Android size budget. Heavier OOXML runtimes such as `ooxmlsdk`, layout-oriented renderers, and future editing/round-trip logic belong in optional Office plugins that users install only when they need richer fidelity.

The Android host has two plugin ABIs. Media plugins implement `ViewItPlugin` and expose `transcode(input, output, progress)`. Office/document plugins implement `ViewItDocumentPlugin`, which is separate so Office plugins do not pretend to be media transcoders.

Initial document-view ABI shape:

```kotlin
interface ViewItDocumentPlugin {
    val id: String
    val version: String
    val supportedFormats: List<String>

    fun initialize(context: Context)
    fun canHandle(mimeType: String): Boolean
    fun canHandleExt(ext: String): Boolean
    fun render(input: Uri, ext: String): String
    fun cleanup()
}
```

`render` returns ViewIt-owned `Document` JSON. Future iterations can add `inspect`, streaming extracted assets, render options, and safe write/export methods. Editing must use separate explicit methods such as `exportCopy` or `saveAs`, never silent in-place mutation.

The first optional Office package is `plugins/office-ooxml`, which keeps `ooxmlsdk` isolated outside the base workspace. It renders DOCX, XLSX, and PPTX into ViewIt-owned `Document` JSON through the document plugin ABI. The catalog ships ABI-specific ZIPs and pins SHA-256 checksums externally, because a ZIP cannot contain a stable checksum of itself.

## FFmpeg Direction

The in-process FFmpegKit Dex/JNI plugin is intentionally not published in the default catalog. It remains in `plugins/ffmpeg-transcoder` as a failed/prototype spike, but users should not be offered it until the runtime model changes.

The safe direction is a host-owned external media runtime, not arbitrary JNI inside a downloaded Dex plugin:

- Desktop: spawn a bundled or user-configured `ffmpeg` executable through a tightly scoped transcoding command.
- Android: prefer platform decoders/native player first; for unsupported formats, use an out-of-process bound service or app-extension package that owns its native libraries at install time.
- Downloadable media packages should provide codecs/assets/configuration to a host-owned loader only when Android can load them deterministically for the current process ABI.
- The UI should continue to expose `Open with another app` for unsupported legacy media until a safe runtime is available.

This avoids the observed FFmpegKit failure mode where Java code loads through `DexClassLoader`, but native class lookup from FFmpegKit resolves through boot/system classloaders and crashes or fails with `NoClassDefFoundError`.

### Excel Reader

The XLSX plugin should parse Office Open XML as a workbook, not as CSV.

Required reader features:

- sheet tabs
- cells, merged cells, row/column dimensions
- number/date formatting
- formulas as displayed cached values, with formula text available
- styles, fills, borders, alignment
- images anchored to cells
- charts rendered as placeholders first, then chart model rendering
- comments and hyperlinks
- freeze panes and basic filters

Recommended implementation path:

- Optional Office OOXML plugin using `ooxmlsdk` for package/relationship/schema access, with `calamine` still allowed where it is smaller or better for raw cell values.
- Svelte renderer for polished spreadsheet UI with virtualized rows/columns.
- Keep advanced chart rendering separate and lazy-loaded.

### Word Reader

The DOCX plugin should parse WordprocessingML structurally.

Required reader features:

- paragraphs and runs
- headings and styles
- tables
- images and captions
- hyperlinks
- lists
- footnotes/endnotes as follow-up sections
- page breaks and section breaks as visual separators
- comments and tracked changes as optional annotations

Recommended implementation path:

- Optional Office OOXML plugin using `ooxmlsdk` for package/relationship/schema access.
- Keep the base app's built-in DOCX path lightweight and best-effort.
- Svelte renderer that maps blocks to semantic HTML with document-width layout.
- Keep editing/writing separate from reading; writing requires a stricter model and ZIP relationship preservation.

## Write Plugins

Write plugins are higher-risk than readers. They need:

- explicit user consent per write/export action
- dry-run validation
- backups or save-as by default
- declared output formats
- an integrity report for unsupported features that may be dropped

For Office formats, write support should preserve unknown OOXML parts and relationships instead of round-tripping through a lossy simplified model.

## Multi-Stream Media Handling

Media plugins must report stream inventory before transcoding:

- video present/missing
- audio present/missing
- subtitles present/missing
- duration
- codec list

The player UI must handle:

- video-only media: play video without audio warning noise
- audio-only media: route to audio player UI
- no audio and no video: show unsupported/corrupt media message
- transcode failure: show plugin log summary and keep the install/open-plugin action available

This should be a host-level contract, not ad hoc plugin behavior.
