#!/usr/bin/env bash
# Two-pass Android release: Rust (fmt-everything) + Gradle, skip RustPlugin re-invoke.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MOBILE="$ROOT/apps/mobile"
GEN="$MOBILE/src-tauri/gen/android"
NDK="${ANDROID_NDK_HOME:-${NDK_HOME:-$HOME/Android/Sdk/ndk/26.1.10909125}}"
JAVA_HOME="${JAVA_HOME:-/usr/lib/jvm/java-21-openjdk-amd64}"
export JAVA_HOME
PDFIUM_CACHE="${ROOT}/.cache/pdfium-android-arm64"
JNI="$GEN/app/src/main/jniLibs/arm64-v8a"
FLAVOR_JNI="$GEN/app/src/arm64/jniLibs/arm64-v8a"

chmod +x "$ROOT/scripts/patch-pdfium-render.sh" "$ROOT/scripts/patch-tauri-android-protocol.sh"
"$ROOT/scripts/patch-pdfium-render.sh"
"$ROOT/scripts/patch-tauri-android-protocol.sh"

export TAURI_ANDROID_PROJECT_PATH="$GEN"

# Ensure gen/android exists (tauri android init) then overlay MainActivity
if [[ ! -d "$GEN/app/src/main/java" ]]; then
  echo "[android] init android project (first-time gen)"
  (cd "$MOBILE" && npm run tauri -- android init 2>/dev/null || true)
fi
chmod +x "$ROOT/scripts/patch-android-mainactivity.sh"
"$ROOT/scripts/patch-android-mainactivity.sh"
export WRY_ANDROID_PACKAGE="ai.viewit.app"

echo "[android] frontend build"
(cd "$MOBILE" && npm run build)

BUILD_PROFILE="${BUILD_PROFILE:-lite}"
if [[ "$BUILD_PROFILE" == "full" ]]; then
  CARGO_FEATURES="fmt-everything,tauri/custom-protocol"
  echo "[android] fetch libpdfium.so (ADR 0002 full build)"
  mkdir -p "$PDFIUM_CACHE" "$JNI"
  if [[ ! -f "$PDFIUM_CACHE/libpdfium.so" ]]; then
    curl -fsSL -o "$PDFIUM_CACHE/pdfium.tgz" \
      "https://github.com/bblanchon/pdfium-binaries/releases/latest/download/pdfium-android-arm64.tgz"
    tar xzf "$PDFIUM_CACHE/pdfium.tgz" -C "$PDFIUM_CACHE" lib/libpdfium.so
    mv "$PDFIUM_CACHE/lib/libpdfium.so" "$PDFIUM_CACHE/libpdfium.so"
  fi
  cp "$PDFIUM_CACHE/libpdfium.so" "$JNI/libpdfium.so"
else
  CARGO_FEATURES="fmt-mobile,tauri/custom-protocol"
  echo "[android] lite build (ADR 0010 — WebView PDF, no libpdfium)"
  mkdir -p "$JNI"
  rm -f "$JNI/libpdfium.so"
fi

LINKER="$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android24-clang"
export ANDROID_NDK_HOME="$NDK"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$LINKER"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS="-C link-arg=-Wl,-z,max-page-size=16384 -C force-unwind-tables=no"
export PATH="$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin:$PATH"
BUILD_TOOLS=$(ls -d "$HOME/Android/Sdk/build-tools/"* 2>/dev/null | sort -V | tail -1)

echo "[android] cargo release lib ($CARGO_FEATURES)"
(cd "$MOBILE" && cargo build --target aarch64-linux-android --release -p viewit-mobile --lib \
  --features "$CARGO_FEATURES")

TARGET_DIR=$(cd "$ROOT" && cargo metadata --format-version 1 --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
RUST_LIB="$TARGET_DIR/aarch64-linux-android/release/libviewit_mobile_lib.so"
if [[ ! -f "$RUST_LIB" ]]; then
  echo "[android] missing Rust shared library: $RUST_LIB" >&2
  exit 1
fi
if ! "$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-readelf" -l "$RUST_LIB" | awk '/LOAD/ && $NF != "0x4000" { bad=1 } END { exit bad }'; then
  echo "[android] Rust shared library is not 16 KB ELF-aligned: $RUST_LIB" >&2
  exit 1
fi
mkdir -p "$JNI" "$FLAVOR_JNI"
for DEST in "$JNI/libviewit_mobile_lib.so" "$FLAVOR_JNI/libviewit_mobile_lib.so"; do
  if [[ "$(realpath "$RUST_LIB")" != "$(realpath -m "$DEST")" ]]; then
    cp "$RUST_LIB" "$DEST"
  fi
done

echo "[android] sync frontend into APK assets (WebViewAssetLoader fallback)"
ASSETS="$GEN/app/src/main/assets"
mkdir -p "$ASSETS"
rsync -a --delete "$MOBILE/build/" "$ASSETS/"

echo "[android] gradle arm64-only APK (~11 MB, not 4-ABI universal)"
(cd "$GEN" && ./gradlew :app:assembleArm64Release \
  -PabiList=arm64-v8a \
  -x rustBuildArm64Release -x rustBuildUniversalRelease --no-daemon)

APK_UNSIGNED="$GEN/app/build/outputs/apk/arm64/release/app-arm64-release-unsigned.apk"
APK_WITH_LIB="$ROOT/dist/viewit-android-arm64-release-with-lib.apk"
APK_ALIGNED="$ROOT/dist/viewit-android-arm64-release-aligned.apk"
APK_SIGNED="$ROOT/dist/viewit-android-arm64-release.apk"
APK_LEGACY="$ROOT/dist/viewit-android-universal-debug.apk"
mkdir -p "$ROOT/dist"

KEYSTORE="${ANDROID_DEBUG_KEYSTORE:-$HOME/.android/debug.keystore}"
if [[ ! -f "$KEYSTORE" ]]; then
  mkdir -p "$(dirname "$KEYSTORE")"
  keytool -genkeypair -v -keystore "$KEYSTORE" -storepass android -alias androiddebugkey \
    -keypass android -keyalg RSA -keysize 2048 -validity 10000 \
    -dname "CN=Android Debug,O=Android,C=US"
fi

rm -f "$APK_WITH_LIB" "$APK_ALIGNED" "$APK_SIGNED" "$APK_LEGACY"
cp "$APK_UNSIGNED" "$APK_WITH_LIB"
TMP_LIB_DIR="$ROOT/dist/android-native-lib"
rm -rf "$TMP_LIB_DIR"
mkdir -p "$TMP_LIB_DIR/lib/arm64-v8a"
cp "$RUST_LIB" "$TMP_LIB_DIR/lib/arm64-v8a/libviewit_mobile_lib.so"
OFFICE_LIB="$GEN/app/src/main/jniLibs/arm64-v8a/libviewit_plugin_office_universal.so"
if [[ -f "$OFFICE_LIB" ]]; then
  cp "$OFFICE_LIB" "$TMP_LIB_DIR/lib/arm64-v8a/libviewit_plugin_office_universal.so"
  echo "[android] adding office-universal plugin to APK"
fi
(cd "$TMP_LIB_DIR" && zip -0 -q "$APK_WITH_LIB" lib/arm64-v8a/libviewit_mobile_lib.so lib/arm64-v8a/libviewit_plugin_office_universal.so 2>/dev/null || zip -0 -q "$APK_WITH_LIB" lib/arm64-v8a/libviewit_mobile_lib.so)
"$BUILD_TOOLS/zipalign" -P 16 -f 4 "$APK_WITH_LIB" "$APK_ALIGNED"
"$BUILD_TOOLS/apksigner" sign --ks "$KEYSTORE" --ks-pass pass:android --key-pass pass:android \
  --out "$APK_SIGNED" "$APK_ALIGNED"

"$ROOT/scripts/verify-android-16kb.sh" "$APK_SIGNED"

MAX_APK_BYTES="${MAX_APK_BYTES:-15000000}"
APK_BYTES=$(stat -c '%s' "$APK_SIGNED")
if (( APK_BYTES > MAX_APK_BYTES )); then
  echo "[android] APK size gate failed: $APK_BYTES bytes > $MAX_APK_BYTES bytes" >&2
  exit 1
fi
echo "[android] APK size OK: $APK_BYTES bytes <= $MAX_APK_BYTES bytes"

echo "[android] size budget"
(cd "$ROOT" && (node --experimental-strip-types scripts/size-budget.ts 2>/dev/null || npx tsx scripts/size-budget.ts))

echo ""
echo "Signed APK (USB install): $APK_SIGNED"
echo "  adb install -r \"$APK_SIGNED\""
echo "  adb shell am start -n ai.viewit.app/.MainActivity"
