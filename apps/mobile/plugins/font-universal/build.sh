#!/bin/bash
# Build script for font-universal Android native plugin
# Builds arm64-v8a and x86_64 .so files

set -e

PLUGIN_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CARGO_DIR="${PLUGIN_DIR}/src/main/rust"
OUTPUT_DIR="${PLUGIN_DIR}/build/output"

if [ -z "$ANDROID_NDK_HOME" ]; then
    echo "ERROR: ANDROID_NDK_HOME not set"
    exit 1
fi

TARGETS=("aarch64-linux-android" "x86_64-linux-android")
ABIS=("arm64-v8a" "x86_64")

for target in "${TARGETS[@]}"; do
    rustup target list --installed | grep -q "^${target}$" || rustup target add "$target"
done

for i in "${!TARGETS[@]}"; do
    TARGET="${TARGETS[$i]}"
    ABI="${ABIS[$i]}"

    echo "Building for $TARGET ($ABI)..."

    export CC="${ANDROID_NDK_HOME}/toolchains/llvm/prebuilt/linux-x86_64/bin/${TARGET}${ANDROID_API:-21}-clang"
    export CXX="${ANDROID_NDK_HOME}/toolchains/llvm/prebuilt/linux-x86_64/bin/${TARGET}${ANDROID_API:-21}-clang++"
    export AR="${ANDROID_NDK_HOME}/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ar"
    export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="${CC}"
    export CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER="${CC}"

    cd "$CARGO_DIR"
    cargo build --target "$TARGET" --release
    TARGET_DIR="$(cargo metadata --format-version 1 --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"

    mkdir -p "${OUTPUT_DIR}/${ABI}"
    cp "${TARGET_DIR}/${TARGET}/release/libviewit_plugin_font_universal.so" "${OUTPUT_DIR}/${ABI}/libviewit_plugin_font_universal.so"

    echo "Built ${OUTPUT_DIR}/${ABI}/libviewit_plugin_font_universal.so"
done

PACKAGE_DIR="${OUTPUT_DIR}/package"
rm -rf "${PACKAGE_DIR}"
mkdir -p "${PACKAGE_DIR}/lib/arm64-v8a"
mkdir -p "${PACKAGE_DIR}/lib/x86_64"
mkdir -p "${PACKAGE_DIR}/dex"

cp "${OUTPUT_DIR}/arm64-v8a/libviewit_plugin_font_universal.so" "${PACKAGE_DIR}/lib/arm64-v8a/"
cp "${OUTPUT_DIR}/x86_64/libviewit_plugin_font_universal.so" "${PACKAGE_DIR}/lib/x86_64/"

cp "${PLUGIN_DIR}/plugin.json" "${PACKAGE_DIR}/"

if command -v d8 &> /dev/null; then
    echo "Generating classes.dex..."
    cd "${PLUGIN_DIR}/src/main/java"
    d8 --output "${PACKAGE_DIR}/dex" --lib "${ANDROID_SDK_HOME}/platforms/android-36/android.jar" ai/viewit/plugins/fontuniversal/FontUniversalPlugin.kt
else
    echo "WARNING: d8 not found, skipping classes.dex generation"
fi

for ABI in "${ABIS[@]}"; do
    rm -f "${OUTPUT_DIR}/font-universal-0.1.0-${ABI}.zip"
    (cd "${PACKAGE_DIR}" && zip -r "${OUTPUT_DIR}/font-universal-0.1.0-${ABI}.zip" "lib/${ABI}" "plugin.json" "dex")
done

echo "Build complete! Output in ${OUTPUT_DIR}"
