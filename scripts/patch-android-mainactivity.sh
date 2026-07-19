#!/usr/bin/env bash
# Keep custom MainActivity (intent → pending opens file) after tauri android gen.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SRC="$ROOT/apps/mobile/src-tauri/android/MainActivity.kt"
DST="$ROOT/apps/mobile/src-tauri/gen/android/app/src/main/java/ai/viewit/app/MainActivity.kt"
if [[ ! -f "$SRC" ]]; then
  echo "[patch-mainactivity] missing $SRC"
  exit 1
fi
mkdir -p "$(dirname "$DST")"
cp "$SRC" "$DST"
echo "[patch-mainactivity] installed $DST"