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

## Live dev (optional)

```bash
npm run dev:android
```

Requires emulator or USB device; uses debug build + hot reload (different from release APK above).