#!/bin/bash
# Build script for iwork-universal Android native plugin
# Builds arm64-v8a and x86_64 .so files with 16 KB page alignment

set -e

PLUGIN_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CARGO_DIR="${PLUGIN_DIR}"
OUTPUT_DIR="${PLUGIN_DIR}/build/output"
ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-${NDK_HOME:-$HOME/Android/Sdk/ndk/26.1.10909125}}"

if [ ! -d "$ANDROID_NDK_HOME" ]; then
    echo "ERROR: ANDROID_NDK_HOME not found at $ANDROID_NDK_HOME"
    exit 1
fi

TARGETS=("aarch64-linux-android" "x86_64-linux-android")
ABIS=("arm64-v8a" "x86_64")

for target in "${TARGETS[@]}"; do
    rustup target list --installed | grep -q "^${target}$" || rustup target add "$target"
done

export RUSTFLAGS="-C link-arg=-Wl,-z,max-page-size=16384"

for i in "${!TARGETS[@]}"; do
    TARGET="${TARGETS[$i]}"
    ABI="${ABIS[$i]}"

    echo "Building iwork-universal for $TARGET ($ABI)..."

    export CC="${ANDROID_NDK_HOME}/toolchains/llvm/prebuilt/linux-x86_64/bin/${TARGET}24-clang"
    export CXX="${ANDROID_NDK_HOME}/toolchains/llvm/prebuilt/linux-x86_64/bin/${TARGET}24-clang++"
    export AR="${ANDROID_NDK_HOME}/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ar"
    export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="${CC}"
    export CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER="${CC}"

    cd "$CARGO_DIR"
    cargo build --target "$TARGET" --release
    TARGET_DIR="$(cargo metadata --format-version 1 --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"

    mkdir -p "${OUTPUT_DIR}/${ABI}"
    cp "${TARGET_DIR}/${TARGET}/release/libviewit_plugin_iwork_universal.so" "${OUTPUT_DIR}/${ABI}/libviewit_plugin_iwork_universal.so"

    echo "Built ${OUTPUT_DIR}/${ABI}/libviewit_plugin_iwork_universal.so"
done

echo "iwork-universal build complete! Output in ${OUTPUT_DIR}"
