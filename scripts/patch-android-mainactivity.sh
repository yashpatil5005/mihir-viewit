#!/usr/bin/env bash
# Keep custom Android sources after tauri android gen.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SRC_DIR="$ROOT/apps/mobile/src-tauri/android"
DST_DIR="$ROOT/apps/mobile/src-tauri/gen/android/app/src/main/java/ai/viewit/app"
SRC_RES="$SRC_DIR/res"
DST_RES="$ROOT/apps/mobile/src-tauri/gen/android/app/src/main/res"
SRC_JNI="$SRC_DIR/jniLibs"
DST_JNI="$ROOT/apps/mobile/src-tauri/gen/android/app/src/main/jniLibs"
SRC_JAVA="$SRC_DIR/app/src/main/java"
DST_JAVA="$ROOT/apps/mobile/src-tauri/gen/android/app/src/main/java"
if [[ ! -d "$SRC_DIR" ]]; then
  echo "[patch-android] missing $SRC_DIR"
  exit 1
fi
mkdir -p "$DST_DIR"
cp "$SRC_DIR"/*.kt "$DST_DIR/"
if [[ -d "$SRC_RES" ]]; then
  cp -R "$SRC_RES"/* "$DST_RES/"
fi
if [[ -d "$SRC_JNI" ]]; then
  cp -R "$SRC_JNI"/* "$DST_JNI/"
fi
if [[ -d "$SRC_JAVA" ]]; then
  cp -R "$SRC_JAVA"/* "$DST_JAVA/"
fi
echo "[patch-android] installed Android source overlays"
