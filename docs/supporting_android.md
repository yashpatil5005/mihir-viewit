# Android Compatibility & WebView Support Strategy

## 1. Supported Android Versions & Target Baseline

- **Minimum SDK (`minSdk`):** `24` (Android 7.0 / 7.1 Nougat)
- **Target SDK (`targetSdk`):** `35` (Android 15)
- **Primary Test Device:** Samsung Galaxy (SM-E346B, `RZCW71NK7SN`), Android 16 (`compileSdk 35/36`)
- **Compatibility Target:** Every active Android version still supported by Google Play Services and system WebViews (Android 7.0+ / API 24 through Android 16+ / API 36).

---

## 2. The Core Problem: WebView Fragmentation & Native Codec Divergence

### The Reality of System WebViews
Across different Android devices and OS versions (Android 7 to 16):
1. **Android System WebView versions vary wildly**: Devices may run anything from Chromium 53 up to Chromium 130+.
2. **Missing Codecs / Container Support**:
   - WebViews and Android native media frameworks on earlier Android versions **do not support Apple-proprietary containers and codecs** natively:
     - **Audio**: `.caf` (Core Audio Format), `.aiff`/`.aif` (AIFF big-endian PCM), `.m4r` (iPhone ringtone AAC), ALAC lossless inside `.m4a`/`.caf`.
     - **Image / Icons**: `.icns` (Apple Icon format with macOS icon chunking), multi-res bitmap masks.
     - **Video**: `.mov` (QuickTime with ProRes or specific Apple tracks).
     - **Localization & Metadata**: `.strings` (Apple NeXTSTEP / UTF-16 strings), `.plist` (Binary or XML Property Lists).
     - **Documents**: `.pages`, `.numbers`, `.key` (iWork packages with `.iwa` protocol buffers / Snappy compression).

### The Architecture Rule
> **Plugins must never assume the Android host or system WebView provides native decoding or rendering for non-standard formats.**
>
> If a plugin claims support for a format in its catalog or manifest, the plugin (or the format crate) **must provide its own self-contained decoder / transpiler** to produce standard web primitives (`ImageData`, Canvas pixels, WAV/PCM audio buffers, DOM nodes, or SVG) rather than passing raw proprietary blobs to native `<video>`, `<audio>`, or `<img>` elements.

---

## 3. Core App vs. Plugin Boundary for Apple Formats

### A. Base App (Built-in / Core Crate) Capabilities
The base application (`viewit-core` + `viewit-fmt-image`) provides offline standalone decoding for standard formats:
- **Plain / Structured Text**: `.plist` (XML), `.strings` (UTF-8/UTF-16 key-value), `.txt`, `.md`, `.json`.
- **Standard Media**: `.mp4`, `.m4v`, `.m4a` (standard AAC/ALAC playable via Android media pipeline / HTML5).
- **Core iWork (Phase 4.1 lightweight)**: `crates/fmt-iwork` extractable text, QuickLook preview rendering for `.pages`, `.numbers`, and `.key`.

### B. Self-Contained Plugin System Capabilities
When running as independent downloadable plugins (or fallback WASM packages), plugins must operate **fully standalone**:
1. **`iwork-universal` Plugin**:
   - Self-contained parser for Apple Pages, Numbers, and Keynote.
   - Extracts QuickLook images, sheet tables, slide hierarchies, and plaintext without requiring Apple binaries or modern Chromium PDF/image features.
2. **Audio & Media Plugins**:
   - Decoding `.caf`, `.aif`/`.aiff`, and ALAC directly into standard linear PCM / WebAudio buffers so older Android WebViews play them identically to modern flagships.
3. **Image & Icon Decoders**:
   - `.icns` decoding with full icon family unpacker (`ic07`, `ic08`, `ic09`, `ic10`, `icp4`, `icp5`, etc.) into RGBA pixel buffers.

---

## 4. Verification Checklist Across Android Releases

When validating file opening on Android (via ADB Intents, Content Providers, and In-App Pickers):

| Format | Android 7-9 (Legacy) | Android 10-12 (Scoped Storage) | Android 13-16 (Granular Media Permissions) |
| :--- | :--- | :--- | :--- |
| `.pages` / `.numbers` / `.key` | WASM / Rust Crate Decoded | WASM / Rust Crate Decoded | WASM / Rust Crate Decoded |
| `.icns` | Custom Rust RGBA decoder | Custom Rust RGBA decoder | Custom Rust RGBA decoder |
| `.caf` / `.aiff` | Transcoded to PCM/WAV buffer | Transcoded to PCM/WAV buffer | Transcoded to PCM/WAV buffer |
| `.plist` / `.strings` | Parsed & highlighted syntax | Parsed & highlighted syntax | Parsed & highlighted syntax |
| `.mov` / `.m4v` | Standard H.264/AAC | Standard H.264/AAC | Standard H.264/AAC |
