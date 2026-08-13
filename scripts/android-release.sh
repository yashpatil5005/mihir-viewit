#!/usr/bin/env bash
# Android release: frontend + Rust lib (canonical flags from
# apps/mobile/.cargo/config.toml) + Gradle, then zip/zipalign/apksigner.
#
# Phase timings are printed by `mark`, so a slow step is visible instead of a
# silent wall-clock gap ("gradle says 32s, whole script takes 10 min").
#
# Rust linker + RUSTFLAGS are NOT overridden here — they live in
# apps/mobile/.cargo/config.toml so Android Studio / gradle rust builds and this
# script share one fingerprint. Any divergence forces a full aarch64 rebuild of
# the dependency tree every time you switch entry points (~10 min).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MOBILE="$ROOT/apps/mobile"
GEN="$MOBILE/src-tauri/gen/android"
NDK="${ANDROID_NDK_HOME:-${NDK_HOME:-$HOME/Android/Sdk/ndk/26.1.10909125}}"
JAVA_HOME="${JAVA_HOME:-/usr/lib/jvm/java-21-openjdk-amd64}"
export JAVA_HOME
export TMPDIR="${VIEWIT_TMPDIR:-$ROOT/build/tmp}"
mkdir -p "$TMPDIR"
export JAVA_TOOL_OPTIONS="${JAVA_TOOL_OPTIONS:-} -Djava.io.tmpdir=$TMPDIR"
PDFIUM_CACHE="${ROOT}/.cache/pdfium-android-arm64"
JNI="$GEN/app/src/main/jniLibs/arm64-v8a"
FLAVOR_JNI="$GEN/app/src/arm64/jniLibs/arm64-v8a"

_ts=$(date +%s%N)
mark() { local now; now=$(date +%s%N); echo "  [$(awk -v a="$now" -v b="$_ts" 'BEGIN{printf "%.1fs", (a-b)/1e9}')] $1"; _ts=$now; }

chmod +x "$ROOT/scripts/patch-pdfium-render.sh" "$ROOT/scripts/patch-tauri-android-protocol.sh"
"$ROOT/scripts/patch-pdfium-render.sh"
"$ROOT/scripts/patch-tauri-android-protocol.sh"
mark "patches applied"

export TAURI_ANDROID_PROJECT_PATH="$GEN"

# Ensure gen/android exists (tauri android init) then overlay MainActivity
if [[ ! -d "$GEN/app/src/main/java" ]]; then
  echo "[android] init android project (first-time gen)"
  (cd "$MOBILE" && npm run tauri -- android init 2>/dev/null || true)
fi
chmod +x "$ROOT/scripts/patch-android-mainactivity.sh"
"$ROOT/scripts/patch-android-mainactivity.sh"
export WRY_ANDROID_PACKAGE="ai.viewit.app"
mark "mainactivity overlay"

# Frontend rebuilds touch frontendDist (apps/mobile/build/), which tauri_build
# mtime-watches — every npm rebuild forces the full aarch64 crate recompile
# (~50 s) on the next cargo run even with zero changes. Hash the frontend
# inputs and skip the npm rebuild when nothing changed since the last build.
FE_STAMP="${VIEWIT_CACHE:-${XDG_CACHE_HOME:-$HOME/.cache}/viewit}/mobile-frontend-inputs.v1.sha"
fe_hash() {
  {
    for p in "$MOBILE/src" "$MOBILE/static" "$MOBILE/package.json" \
             "$MOBILE/svelte.config.js" "$MOBILE/vite.config.ts" "$MOBILE/tsconfig.json" \
             "$ROOT/packages/ui/src" "$ROOT/packages/ui/package.json" "$ROOT/packages/ui/tsconfig.json"; do
      if [[ -f "$p" ]]; then
        sha256sum "$p"
      elif [[ -d "$p" ]]; then
        find "$p" -type f -print0 | sort -z | xargs -0 sha256sum
      fi
    done
  } | sha256sum | awk '{print $1}'
}
FE_HASH=$(fe_hash)
if [[ -f "$FE_STAMP" && "$(cat "$FE_STAMP")" == "$FE_HASH" ]]; then
  echo "[android] frontend inputs unchanged — skipping npm rebuild"
else
  echo "[android] frontend build"
  (cd "$MOBILE" && npm run build)
  mkdir -p "$(dirname "$FE_STAMP")"
  printf '%s\n' "$FE_HASH" > "$FE_STAMP"
fi
mark "frontend build"

APP_VERSION=$(python3 -c "import json;print(json.load(open('$MOBILE/src-tauri/tauri.conf.json'))['version'])")
IFS=. read -r VMAJOR VMINOR VPATCH <<< "${APP_VERSION%%-*}"
VERSION_CODE=$((10#$VMAJOR * 1000000 + 10#$VMINOR * 1000 + 10#$VPATCH))
cat > "$GEN/app/tauri.properties" <<EOF
tauri.android.versionName=$APP_VERSION
tauri.android.versionCode=$VERSION_CODE
EOF
echo "[android] version $APP_VERSION ($VERSION_CODE)"
mark "android version"

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

export ANDROID_NDK_HOME="$NDK"
export PATH="$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin:$PATH"
BUILD_TOOLS=$(ls -d "$HOME/Android/Sdk/build-tools/"* 2>/dev/null | sort -V | tail -1)

# VIEWIT_SKIP_RUST=1 reuses the previously built .so (iterating on frontend /
# gradle / plugin packaging only, no mobile Rust changes). The ELF-alignment
# gate below still runs, so a misaligned stale lib fails loudly.
if [[ "${VIEWIT_SKIP_RUST:-0}" == "1" ]]; then
  echo "[android] VIEWIT_SKIP_RUST=1 — reusing previous Rust lib (no cargo)"
else
  echo "[android] cargo release lib ($CARGO_FEATURES)"
  (cd "$MOBILE" && cargo build --target aarch64-linux-android --release -p viewit-mobile --lib \
    --features "$CARGO_FEATURES")
fi
mark "cargo aarch64 release lib"

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

# ALL feature plugins (office / compression / font universal + pptx-vanilla) are
# NOW downloadable catalog plugins — nothing is baked into the APK. Remove any
# plugin .so that a previous build staged into jniLibs so it does not leak in.

for DEST in "$JNI" "$FLAVOR_JNI"; do
  rm -f "$DEST/libviewit_plugin_office_universal.so" \
        "$DEST/libviewit_plugin_compression_universal.so" \
        "$DEST/libviewit_plugin_font_universal.so"
done

echo "[android] sync frontend into APK assets (WebViewAssetLoader fallback)"
ASSETS="$GEN/app/src/main/assets"
mkdir -p "$ASSETS"
rsync -a --delete "$MOBILE/build/" "$ASSETS/"
mark "frontend assets sync"

echo "[android] gradle arm64-only APK (~11 MB, not 4-ABI universal)"
GRADLE_EXTRA=""
# CI: hermetic single-run JVM. Local: reuse the gradle daemon (much faster
# repeated builds, and it's what makes warm runs seconds instead of minutes).
[[ "${CI:-0}" == "1" ]] && GRADLE_EXTRA="--no-daemon"
(cd "$GEN" && ./gradlew :app:assembleArm64Release :app:bundleArm64Release \
  -PabiList=arm64-v8a \
  -x rustBuildArm64Release -x rustBuildUniversalRelease $GRADLE_EXTRA)
mark "gradle assembleArm64Release + bundleArm64Release"

APK_UNSIGNED="$GEN/app/build/outputs/apk/arm64/release/app-arm64-release-unsigned.apk"
APK_WITH_LIB="$ROOT/dist/viewit-android-arm64-release-with-lib.apk"
APK_ALIGNED="$ROOT/dist/viewit-android-arm64-release-aligned.apk"
APK_SIGNED="$ROOT/dist/viewit-android-arm64-release.apk"
APK_LEGACY="$ROOT/dist/viewit-android-universal-debug.apk"
mkdir -p "$ROOT/dist"

if [[ "${VIEWIT_PRODUCTION_SIGNING:-0}" == "1" ]]; then
  KEYSTORE="${ANDROID_SIGNING_KEYSTORE:?ANDROID_SIGNING_KEYSTORE required}"
  KS_PASS="${ANDROID_SIGNING_STORE_PASS:?ANDROID_SIGNING_STORE_PASS required}"
  KEY_PASS="${ANDROID_SIGNING_KEY_PASS:?ANDROID_SIGNING_KEY_PASS required}"
  KEY_ALIAS="${ANDROID_SIGNING_KEY_ALIAS:?ANDROID_SIGNING_KEY_ALIAS required}"
  [[ -f "$KEYSTORE" ]] || { echo "[android] production keystore not found: $KEYSTORE" >&2; exit 1; }
else
  KEYSTORE="${ANDROID_DEBUG_KEYSTORE:-$HOME/.android/debug.keystore}"
  KS_PASS=android
  KEY_PASS=android
  KEY_ALIAS=androiddebugkey
  if [[ ! -f "$KEYSTORE" ]]; then
    mkdir -p "$(dirname "$KEYSTORE")"
    keytool -genkeypair -v -keystore "$KEYSTORE" -storepass "$KS_PASS" -alias "$KEY_ALIAS" \
      -keypass "$KEY_PASS" -keyalg RSA -keysize 2048 -validity 10000 \
      -dname "CN=Android Debug,O=Android,C=US"
  fi
fi

cp "$APK_UNSIGNED" "$APK_WITH_LIB"
TMP_LIB_DIR="$ROOT/dist/android-native-lib"
rm -rf "$TMP_LIB_DIR"
mkdir -p "$TMP_LIB_DIR/lib/arm64-v8a"
cp "$RUST_LIB" "$TMP_LIB_DIR/lib/arm64-v8a/libviewit_mobile_lib.so"
# Feature plugins are downloadable catalog plugins; the native-lib staging dir
# carries only the core mobile lib, so no plugin .so ships inside any artifact.
# The main Rust lib must stay stored + 16 KB-aligned (verify-android-16kb.sh).
# Plugin .so files load via System.loadLibrary from the extracted native-lib
# dir (useLegacyPackaging=true), so they can be deflated to save APK budget.
(cd "$TMP_LIB_DIR" && zip -0 -q "$APK_WITH_LIB" lib/arm64-v8a/libviewit_mobile_lib.so \
  && zip -9 -q "$APK_WITH_LIB" lib/arm64-v8a/libviewit_plugin_office_universal.so lib/arm64-v8a/libviewit_plugin_compression_universal.so lib/arm64-v8a/libviewit_plugin_font_universal.so 2>/dev/null \
  || zip -0 -q "$APK_WITH_LIB" lib/arm64-v8a/libviewit_mobile_lib.so)
"$BUILD_TOOLS/zipalign" -P 16 -f 4 "$APK_WITH_LIB" "$APK_ALIGNED"
"$BUILD_TOOLS/apksigner" sign --ks "$KEYSTORE" --ks-key-alias "$KEY_ALIAS" \
  --ks-pass "pass:$KS_PASS" --key-pass "pass:$KEY_PASS" \
  --out "$APK_SIGNED" "$APK_ALIGNED"
mark "zip/zipalign/apksigner"

"$ROOT/scripts/verify-android-16kb.sh" "$APK_SIGNED"

# App Bundle for Google Play upload (Play App Signing re-signs the APKs it
# generates from this AAB, so the AAB is signed with the same upload key).
AAB_UNSIGNED="$GEN/app/build/outputs/bundle/arm64/release/app-arm64-release.aab"
[[ -f "$AAB_UNSIGNED" ]] || AAB_UNSIGNED="$GEN/app/build/outputs/bundle/arm64Release/app-arm64-release.aab"
AAB_SIGNED="$ROOT/dist/viewit-android-arm64-release.aab"
if [[ -f "$AAB_UNSIGNED" ]]; then
  # AABs are Zip/JAR-based, so jarsigner signs them (apksigner is APK-only).
  jarsigner -sigalg SHA256withRSA -digestalg SHA-256 \
    -keystore "$KEYSTORE" -storepass "$KS_PASS" -keypass "$KEY_PASS" \
    -signedjar "$AAB_SIGNED" "$AAB_UNSIGNED" "$KEY_ALIAS"
  mark "aab jarsigner"
  echo "Play App Bundle (upload): $AAB_SIGNED"
else
  echo "[android] warn: bundle not found at $AAB_UNSIGNED (APK-only build)" >&2
fi

MAX_APK_BYTES="${MAX_APK_BYTES:-15000000}"
APK_BYTES=$(stat -c '%s' "$APK_SIGNED")
if (( APK_BYTES > MAX_APK_BYTES )); then
  echo "[android] APK size gate failed: $APK_BYTES bytes > $MAX_APK_BYTES bytes" >&2
  exit 1
fi
echo "[android] APK size OK: $APK_BYTES bytes <= $MAX_APK_BYTES bytes"

echo "[android] size budget"
(cd "$ROOT" && (node --experimental-strip-types scripts/size-budget.ts 2>/dev/null || timeout 90 npx tsx scripts/size-budget.ts))
mark "size budget"

echo ""
echo "Signed APK (USB install): $APK_SIGNED"
echo "  adb install -r \"$APK_SIGNED\""
