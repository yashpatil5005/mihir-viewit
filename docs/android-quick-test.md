# Test ViewIt on your phone (after `adb install`)

## Install (from dev machine)

```bash
adb devices   # must show "device"
adb install -r dist/viewit-android-arm64-release.apk
adb shell am start -n ai.viewit.app/.MainActivity
```

Use `adb shell pm clear ai.viewit.app` after installation when a clean plugin/runtime state is required; Android backup restore can otherwise restore old app-private plugins.

Rebuild + install:

```bash
VIEWIT_APP_PROFILE=device-test BUILD_PROFILE=lite JAVA_HOME=/usr/lib/jvm/java-21-openjdk-amd64 npm run build:android-release
adb install -r dist/viewit-android-arm64-release.apk
```

## On the phone

1. **Open with** — Gallery/Files → image or PDF → Share/Open with → **ViewIt**.
2. **In-app picker** — ViewIt → **Open file…**
3. **Debug** — tap **▸ log** (top). After opening a file you should see lines like:
   - `openFile content://…`
   - `ok kind=image` / `ok kind=media` / `ok kind=stream-file` / `ok kind=text`
   - `openFile err: …` if Rust failed

## From PC (optional)

```bash
./scripts/adb-log-viewit.sh
```

Reproduce on phone; watch `RustStdoutStderr` / errors.

## What was wrong (2026-07-15)

Materialized `asset_path` was passed to `<img>` as raw `file://` without **`convertFileSrc`** — fixed in `packages/platform/src/mediaUrl.ts`. Reinstall APK after that change.
