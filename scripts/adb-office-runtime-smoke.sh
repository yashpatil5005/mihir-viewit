#!/usr/bin/env bash
# Reproducible Android smoke for the optional Office OOXML document runtime.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APK="${APK:-$ROOT/dist/viewit-android-universal-debug.apk}"
PKG="ai.viewit.app"
CATALOG_PORT="${CATALOG_PORT:-8888}"
CATALOG_URL="http://127.0.0.1:$CATALOG_PORT/catalog.json"

DOCX="${DOCX:-/sdcard/Download/viewit-sample.docx}"
XLSX="${XLSX:-/sdcard/Download/viewit-sample.xlsx}"
PPTX="${PPTX:-/sdcard/Download/viewit-sample.pptx}"

require() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 2
  fi
}

launch_file() {
  local path="$1"
  local mime="$2"
  echo "[office-smoke] opening $path"
  adb shell am start \
    -a android.intent.action.VIEW \
    -d "file://$path" \
    -t "$mime" \
    -n "$PKG/.MainActivity" >/dev/null
}

require adb
require python3

if [[ ! -f "$APK" ]]; then
  echo "APK not found: $APK" >&2
  echo "Build it first with: bash scripts/android-release.sh" >&2
  exit 2
fi

echo "[office-smoke] installing $APK"
adb install -r "$APK" >/dev/null
adb reverse "tcp:$CATALOG_PORT" "tcp:$CATALOG_PORT" >/dev/null

echo "[office-smoke] serving plugin catalog at $CATALOG_URL"
(cd "$ROOT/plugins" && python3 -m http.server "$CATALOG_PORT" --bind 127.0.0.1 >/tmp/viewit-plugin-catalog.log 2>&1) &
server_pid=$!
trap 'kill "$server_pid" >/dev/null 2>&1 || true' EXIT

adb shell am force-stop "$PKG" >/dev/null
adb shell am start -n "$PKG/.MainActivity" >/dev/null

echo "[office-smoke] install office-ooxml from Runtime Chooser using catalog: $CATALOG_URL"
echo "[office-smoke] expected for each Office file: Runtime: Enhanced Office OOXML"
read -r -p "Press Enter after office-ooxml is installed on the emulator... "

launch_file "$DOCX" "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
read -r -p "Verify DOCX shows Enhanced Office OOXML, then press Enter... "

launch_file "$XLSX" "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
read -r -p "Verify XLSX shows Enhanced Office OOXML, then press Enter... "

launch_file "$PPTX" "application/vnd.openxmlformats-officedocument.presentationml.presentation"
read -r -p "Verify PPTX shows Enhanced Office OOXML, then press Enter... "

echo "[office-smoke] complete"
