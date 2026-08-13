#!/bin/bash
# Build script for office-universal Android native plugin
# Builds arm64-v8a and x86_64 .so files

set -euo pipefail

PLUGIN_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CARGO_DIR="${PLUGIN_DIR}/src/main/rust"
OUTPUT_DIR="${PLUGIN_DIR}/build/output"

ROOT="$(cd "${PLUGIN_DIR}/../../../.." && pwd)"
ANDROID_HOME="${ANDROID_HOME:-${ANDROID_SDK_HOME:-$HOME/Android/Sdk}}"
ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-${NDK_HOME:-$ANDROID_HOME/ndk/26.1.10909125}}"
export ANDROID_HOME ANDROID_NDK_HOME
[ -d "$ANDROID_NDK_HOME" ] || { echo "ERROR: Android NDK not found: $ANDROID_NDK_HOME"; exit 1; }

# Rust targets for Android
TARGETS=("aarch64-linux-android" "x86_64-linux-android")
ABIS=("arm64-v8a" "x86_64")

# Ensure rustup targets are installed
for target in "${TARGETS[@]}"; do
    rustup target list --installed | grep -q "^${target}$" || rustup target add "$target"
done

# Build for each target
for i in "${!TARGETS[@]}"; do
    TARGET="${TARGETS[$i]}"
    ABI="${ABIS[$i]}"

    echo "Building for $TARGET ($ABI)..."

    # Set up environment for cross-compilation
    export CC="${ANDROID_NDK_HOME}/toolchains/llvm/prebuilt/linux-x86_64/bin/${TARGET}${ANDROID_API:-21}-clang"
    export CXX="${ANDROID_NDK_HOME}/toolchains/llvm/prebuilt/linux-x86_64/bin/${TARGET}${ANDROID_API:-21}-clang++"
    export AR="${ANDROID_NDK_HOME}/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ar"
    export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="${CC}"
    export CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER="${CC}"

    cd "$CARGO_DIR"
    cargo build --target "$TARGET" --release
    TARGET_DIR="$(cargo metadata --format-version 1 --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"

    # Copy the built library
    mkdir -p "${OUTPUT_DIR}/${ABI}"
    cp "${TARGET_DIR}/${TARGET}/release/libviewit_plugin_office_universal.so" "${OUTPUT_DIR}/${ABI}/libviewit_plugin_office_universal.so"

    echo "Built ${OUTPUT_DIR}/${ABI}/libviewit_plugin_office_universal.so"
done

echo "Packaging canonical plugin ZIPs..."
bash "$ROOT/scripts/package-plugins.sh" --plugin office-universal
echo "Build complete! Validated output in ${OUTPUT_DIR}"
