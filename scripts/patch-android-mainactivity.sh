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
SRC_GRADLE="$SRC_DIR/app.build.gradle.kts"
DST_GRADLE="$ROOT/apps/mobile/src-tauri/gen/android/app/build.gradle.kts"
SRC_PROGUARD="$SRC_DIR/viewit.pro"
DST_PROGUARD="$ROOT/apps/mobile/src-tauri/gen/android/app/viewit.pro"
SRC_TESTS="$SRC_DIR/tests"
DST_TESTS="$ROOT/apps/mobile/src-tauri/gen/android/app/src/test/java/ai/viewit/app"
if [[ ! -d "$SRC_DIR" ]]; then
  echo "[patch-android] missing $SRC_DIR"
  exit 1
fi
mkdir -p "$DST_DIR"
cp "$SRC_DIR"/*.kt "$DST_DIR/"
mkdir -p "$DST_TESTS"
rm -f "$DST_TESTS"/*.kt
if [[ -d "$SRC_TESTS" ]]; then
  cp "$SRC_TESTS"/*.kt "$DST_TESTS/"
fi
cp "$SRC_GRADLE" "$DST_GRADLE"
cp "$SRC_PROGUARD" "$DST_PROGUARD"
if [[ -d "$SRC_RES" ]]; then
  cp -R "$SRC_RES"/* "$DST_RES/"
fi
if [[ -d "$SRC_JNI" ]]; then
  cp -R "$SRC_JNI"/* "$DST_JNI/"
fi
if [[ -d "$SRC_JAVA" ]]; then
  cp -R "$SRC_JAVA"/* "$DST_JAVA/"
fi
python3 "$ROOT/scripts/patch-android-open-with.py"
echo "[patch-android] installed Android source overlays"
