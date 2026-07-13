# Android USB test (full build)

## Build (Linux)

```bash
JAVA_HOME=/usr/lib/jvm/java-21-openjdk-amd64 npm run build:android-release
```

Output: `dist/viewit-android-universal-debug.apk` (debug-signed, `adb install` OK).

## Phone setup

1. Developer options → USB debugging on.
2. USB cable → `adb devices` shows device `device` (not `unauthorized` — accept RSA prompt on phone).

## Install & launch

```bash
adb install -r dist/viewit-android-universal-debug.apk
adb shell am start -n ai.viewit.app/.MainActivity
```

## What to exercise

- **Share into ViewIt**: Files / Drive → Share → ViewIt (SEND intent).
- **Open with**: tap `.txt`, `.pdf`, `.json`, `.md`, `.csv`, image → Open with → ViewIt.
- **In-app**: onboarding card, dark mode toggle, open file picker if wired in UI.

`content://` URIs use `tauri-plugin-fs` in `open_uri` (not raw `file://` only).

## Sizes (fmt-everything + libpdfium arm64)

| Artifact | Measured | Budget |
|----------|----------|--------|
| AAB | ~10.4 MB | 18 MB |
| APK | ~17.8 MB | 20 MB |

Native libs in APK: `libviewit_mobile_lib.so` (~8.4 MB), `libpdfium.so` (~6.1 MB).

## Live dev (USB, recommended while iterating UI)

**`chrome://inspect` is on your PC Chrome** (not the phone). USB debugging on → open that URL on the **computer** → find **ViewIt** under Remote Target → Inspect.

Phone + USB debugging on. From **repo root**:

```bash
TAURI_DEV_HOST=$(hostname -I | awk '{print $1}') npm run dev:android
```

Or from `apps/mobile`:

```bash
TAURI_DEV_HOST=$(hostname -I | awk '{print $1}') npm run dev:android
```

Uses Vite on `0.0.0.0:1421`; Tauri proxies dev URL to the phone. **Not** the same as release APK.

## Performance (offline)

- **Video / audio**: rejected from extension before any read (picker + share `probe_uri`).
- **Picker files**: base64 IPC chunk to Rust (`open_bytes_b64`) — not `number[]` JSON.
- **Share `content://`**: still reads full file once (≤32 MB); video blocked by extension on `probe_uri` first.
- **PDF**: 2 eager pages on Android; extra pages via **Load page** (`pdf_page` command).

## Release APK still blank / Internal Server Error?

Rebuild after protocol patch:

```bash
JAVA_HOME=/usr/lib/jvm/java-21-openjdk-amd64 npm run build:android-release
adb install -r dist/viewit-android-universal-debug.apk
```

Chrome inspect WebView: `chrome://inspect` (enabled in `MainActivity` via `WebView.setWebContentsDebuggingEnabled`).

Logcat while launching:

```bash
adb logcat -c && adb shell am force-stop ai.viewit.app
adb shell am start -n ai.viewit.app/.MainActivity
adb logcat | grep -iE 'AssetNotFound|custom protocol timed out|RustStdoutStderr'
```