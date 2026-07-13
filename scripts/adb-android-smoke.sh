#!/usr/bin/env bash
# Quick USB smoke: install signed APK, launch, tail relevant logcat.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APK="${1:-$ROOT/dist/viewit-android-universal-debug.apk}"
adb devices
adb install -r "$APK"
adb shell am force-stop ai.viewit.app
adb logcat -c
adb shell am start -n ai.viewit.app/.MainActivity
echo "Tailing logcat (Ctrl+C to stop)…"
adb logcat | grep -iE 'AssetNotFound|custom protocol timed out|Failed to request|RustStdoutStderr|chromium.*CONSOLE'