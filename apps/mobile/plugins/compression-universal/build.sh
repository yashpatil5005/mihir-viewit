#!/bin/bash
# Build script for compression-universal Android native plugin
# Builds arm64-v8a and x86_64 .so files
#
# C deps (unrar_sys, lzma-sys, bzip2-sys, zstd-sys) need bionic-friendly
# defines — .cargo/config.toml injects them via CFLAGS/CXXFLAGS for the
# Android targets (-D__ANDROID_API__=26 -D_BSD_SOURCE
# -D__ANDROID_UNAVAILABLE_SYMBOLS_ARE_WEAK__=1).

set -e

PLUGIN_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CARGO_DIR="${PLUGIN_DIR}/src/main/rust"
OUTPUT_DIR="${PLUGIN_DIR}/build/output"

# Android NDK setup
if [ -z "$ANDROID_NDK_HOME" ]; then
    echo "ERROR: ANDROID_NDK_HOME not set"
    exit 1
fi
TC="${ANDROID_NDK_HOME}/toolchains/llvm/prebuilt/linux-x86_64/bin"

# Rust targets for Android
TARGETS=("aarch64-linux-android" "x86_64-linux-android")
ABIS=("arm64-v8a" "x86_64")
export ANDROID_API="${ANDROID_API:-24}"

# Ensure rustup targets are installed
for target in "${TARGETS[@]}"; do
    rustup target list --installed | grep -q "^${target}$" || rustup target add "$target"
done

# Build for each target
for i in "${!TARGETS[@]}"; do
    TARGET="${TARGETS[$i]}"
    ABI="${ABIS[$i]}"

    echo "Building for $TARGET ($ABI)..."

    export CC="${TC}/${TARGET}${ANDROID_API}-clang"
    export CXX="${TC}/${TARGET}${ANDROID_API}-clang++"
    export AR="${TC}/llvm-ar"
    # cc crate picks these up when the target env vars are absent; set both.
    export "CC_${TARGET//-/_}"="${CC}"
    export "CXX_${TARGET//-/_}"="${CXX}"
    export "CARGO_TARGET_$(echo "${TARGET//-/_}" | tr '[:lower:]' '[:upper:]')_LINKER"="${CC}"

    (cd "$CARGO_DIR" && cargo build --target "$TARGET" --release)
    TARGET_DIR="$(cd "$CARGO_DIR" && cargo metadata --format-version 1 --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"

    mkdir -p "${OUTPUT_DIR}/${ABI}"
    cp "${TARGET_DIR}/${TARGET}/release/libviewit_plugin_compression_universal.so" "${OUTPUT_DIR}/${ABI}/libviewit_plugin_compression_universal.so"
    echo "Built ${OUTPUT_DIR}/${ABI}/libviewit_plugin_compression_universal.so"
done

# Create plugin package — canonical root layout (same as office-universal):
#   plugin.json / lib/<ABI>/libviewit_plugin_compression_universal.so / dex/
PACKAGE_DIR="${OUTPUT_DIR}/package"
rm -rf "${PACKAGE_DIR}"
mkdir -p "${PACKAGE_DIR}/lib/arm64-v8a"
mkdir -p "${PACKAGE_DIR}/lib/x86_64"
mkdir -p "${PACKAGE_DIR}/dex"

cp "${OUTPUT_DIR}/arm64-v8a/libviewit_plugin_compression_universal.so" "${PACKAGE_DIR}/lib/arm64-v8a/"
cp "${OUTPUT_DIR}/x86_64/libviewit_plugin_compression_universal.so" "${PACKAGE_DIR}/lib/x86_64/"
cp "${PLUGIN_DIR}/plugin.json" "${PACKAGE_DIR}/"

# Generate classes.dex — best-effort, never fatal (same posture as office-universal:
# the Kotlin plugin wrapper is compiled into the host app APK, so the dex in the
# package is only needed for future install-from-catalog flows).
# d8 reads .class/.jar only, so when kotlinc is available we compile first.
if command -v d8 &> /dev/null; then
    echo "Generating classes.dex..."
    SDK="${ANDROID_SDK_HOME:-$HOME/Android/Sdk}"
    ANDROID_JAR=$(ls -d "$SDK"/platforms/android-*/android.jar 2>/dev/null | sort -V | tail -1)
    if [ -z "$ANDROID_JAR" ]; then
        echo "WARNING: no android.jar found under $SDK/platforms — skipping classes.dex"
    elif command -v kotlinc &> /dev/null; then
        (cd "${PLUGIN_DIR}/src/main/java" \
          && kotlinc -classpath "${ANDROID_JAR}" -d "${TMPDIR:-/tmp}/cu-plugin.jar" ai/viewit/plugins/compressionuniversal/CompressionUniversalPlugin.kt \
          && d8 --output "${PACKAGE_DIR}/dex" --lib "$ANDROID_JAR" "${TMPDIR:-/tmp}/cu-plugin.jar" \
          && rm -f "${TMPDIR:-/tmp}/cu-plugin.jar")
    else
        echo "WARNING: kotlinc not found, skipping classes.dex generation"
    fi
else
    echo "WARNING: d8 not found, skipping classes.dex generation"
fi

# Create ZIP packages (one per ABI, entries relative to the package root).
for ABI in "${ABIS[@]}"; do
    rm -f "${OUTPUT_DIR}/compression-universal-0.1.0-${ABI}.zip"
    (cd "${PACKAGE_DIR}" && zip -r "${OUTPUT_DIR}/compression-universal-0.1.0-${ABI}.zip" "lib/${ABI}" "plugin.json" "dex")
done

echo "Build complete! Output in ${OUTPUT_DIR}"
