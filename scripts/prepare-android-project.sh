#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MOBILE="$ROOT/apps/mobile"
MANIFEST="$MOBILE/src-tauri/gen/android/app/src/main/AndroidManifest.xml"

if [[ ! -f "$MANIFEST" ]]; then
  echo "[android] init android project"
  (cd "$MOBILE" && npm run tauri -- android init)
fi
"$ROOT/scripts/patch-android-mainactivity.sh"
