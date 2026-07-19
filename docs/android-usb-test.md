# Android USB test (full build)

## Build (Linux)

Default **lite** (no pdfium, WebView PDF/video stream):

```bash
BUILD_PROFILE=lite JAVA_HOME=/usr/lib/jvm/java-21-openjdk-amd64 npm run build:android-release
```

Full raster PDF (`libpdfium`):

```bash
BUILD_PROFILE=full JAVA_HOME=/usr/lib/jvm/java-21-openjdk-amd64 npm run build:android-release
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

- **Open with / Share → ViewIt**: PDF, image, txt, etc. (custom `MainActivity` queues URIs → Rust on startup).
- **Open file…**: Tauri **dialog** (SAF picker, not broken `<input>`).
- **Browse / Choose file…**: same SAF picker on mobile.
- **In-app**: onboarding card, dark mode toggle, open file picker if wired in UI.

`content://` URIs use `tauri-plugin-fs` in `open_uri` (not raw `file://` only).

## Sizes (fmt-everything-lite, ADR 0010)

| Artifact | Measured | Budget |
|----------|----------|--------|
| AAB | ~7.0 MB | 18 MB |
| APK | ~10.8 MB | 20 MB |

Native libs in APK: `libviewit_mobile_lib.so` only (~8.2 MB). No `libpdfium.so` in lite.

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

- **PDF / video / audio (share)**: `open_stream` — no full read; WebView via `convertFileSrc` + asset protocol.
- **PDF (picker, ≤32 MB)**: may raster via Rust if `fmt-pdf`; lite build uses **iframe** + blob URL.
- **Video (picker, large)**: blob URL + `<video>` without reading into Rust.
- **Docs (txt/xlsx/…)**: share still reads once (≤32 MB); picker uses `open_bytes_b64`.

## Release APK still blank / Internal Server Error?

Rebuild after protocol patch:

```bash
JAVA_HOME=/usr/lib/jvm/java-21-openjdk-amd64 npm run build:android-release
adb install -r dist/viewit-android-universal-debug.apk
```

**No Chrome?** Use in-app **▸ log** (header) for open/stream errors, or PC:

```bash
./scripts/adb-log-viewit.sh
```

Chrome inspect (optional): `chrome://inspect` on a PC with Chrome installed.

Logcat while launching:

```bash
adb logcat -c && adb shell am force-stop ai.viewit.app
adb shell am start -n ai.viewit.app/.MainActivity
adb logcat | grep -iE 'AssetNotFound|custom protocol timed out|RustStdoutStderr'
```