#!/usr/bin/env bash
# Manual inspect without Chrome: tail ViewIt + Rust + WebView errors.
set -euo pipefail
adb logcat -c 2>/dev/null || true
echo "Launch ViewIt, reproduce issue, then watch below (Ctrl+C)…"
adb logcat -v time | grep -iE 'RustStdoutStderr|viewit|register_stream|read failed|open_uri|chromium.*ERROR|AssetNotFound|protocol'