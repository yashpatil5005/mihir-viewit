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

chmod +x "$ROOT/scripts/patch-pdfium-render.sh" "$ROOT/scripts/patch-tauri-android-protocol.sh"
"$ROOT/scripts/patch-pdfium-render.sh"
"$ROOT/scripts/patch-tauri-android-protocol.sh"

export TAURI_ANDROID_PROJECT_PATH="$GEN"

echo "[android] frontend build"
(cd "$MOBILE" && npm run build)

echo "[android] fetch libpdfium.so (ADR 0002)"
mkdir -p "$PDFIUM_CACHE" "$JNI"
if [[ ! -f "$PDFIUM_CACHE/libpdfium.so" ]]; then
  curl -fsSL -o "$PDFIUM_CACHE/pdfium.tgz" \
    "https://github.com/bblanchon/pdfium-binaries/releases/latest/download/pdfium-android-arm64.tgz"
  tar xzf "$PDFIUM_CACHE/pdfium.tgz" -C "$PDFIUM_CACHE" lib/libpdfium.so
  mv "$PDFIUM_CACHE/lib/libpdfium.so" "$PDFIUM_CACHE/libpdfium.so"
fi
cp "$PDFIUM_CACHE/libpdfium.so" "$JNI/libpdfium.so"

LINKER="$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android24-clang"
export ANDROID_NDK_HOME="$NDK"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$LINKER"
export PATH="$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin:$PATH"

echo "[android] cargo release lib (fmt-everything + embedded assets)"
# tauri/custom-protocol: must match Tauri release APK (no http://localhost:1421).
(cd "$MOBILE" && cargo build --target aarch64-linux-android --release -p viewit-mobile --lib \
  --features "fmt-everything,tauri/custom-protocol")

echo "[android] sync frontend into APK assets (WebViewAssetLoader fallback)"
ASSETS="$GEN/app/src/main/assets"
mkdir -p "$ASSETS"
rsync -a --delete "$MOBILE/build/" "$ASSETS/"

echo "[android] gradle AAB + APK"
(cd "$GEN" && ./gradlew :app:bundleUniversalRelease :app:assembleUniversalRelease \
  -x rustBuildArm64Release -x rustBuildUniversalRelease --no-daemon)

echo "[android] size gate"
(cd "$ROOT" && npx tsx scripts/size-budget.ts)

APK_UNSIGNED="$GEN/app/build/outputs/apk/universal/release/app-universal-release-unsigned.apk"
APK_SIGNED="$ROOT/dist/viewit-android-universal-debug.apk"
mkdir -p "$ROOT/dist"

KEYSTORE="${ANDROID_DEBUG_KEYSTORE:-$HOME/.android/debug.keystore}"
if [[ ! -f "$KEYSTORE" ]]; then
  mkdir -p "$(dirname "$KEYSTORE")"
  keytool -genkeypair -v -keystore "$KEYSTORE" -storepass android -alias androiddebugkey \
    -keypass android -keyalg RSA -keysize 2048 -validity 10000 \
    -dname "CN=Android Debug,O=Android,C=US"
fi

BUILD_TOOLS=$(ls -d "$HOME/Android/Sdk/build-tools/"* 2>/dev/null | sort -V | tail -1)
"$BUILD_TOOLS/apksigner" sign --ks "$KEYSTORE" --ks-pass pass:android --key-pass pass:android \
  --out "$APK_SIGNED" "$APK_UNSIGNED"

echo ""
echo "Signed APK (USB install): $APK_SIGNED"
echo "  adb install -r \"$APK_SIGNED\""
echo "  adb shell am start -n ai.viewit.app/.MainActivity"