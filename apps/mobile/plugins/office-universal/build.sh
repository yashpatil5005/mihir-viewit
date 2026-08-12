#!/bin/bash
# Build script for office-universal Android native plugin
# Builds arm64-v8a and x86_64 .so files

set -e

PLUGIN_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CARGO_DIR="${PLUGIN_DIR}/src/main/rust"
OUTPUT_DIR="${PLUGIN_DIR}/build/output"

# Android NDK setup
if [ -z "$ANDROID_NDK_HOME" ]; then
    echo "ERROR: ANDROID_NDK_HOME not set"
    exit 1
fi

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

# Create plugin package — canonical root layout matching office-ooxml:
#   plugin.json / lib/<ABI>/libviewit_plugin_office_universal.so / [dex/]
# (The app's plugin loader expects entries at the ZIP root, NOT under package/.)
PACKAGE_DIR="${OUTPUT_DIR}/package"
rm -rf "${PACKAGE_DIR}"
mkdir -p "${PACKAGE_DIR}/lib/arm64-v8a"
mkdir -p "${PACKAGE_DIR}/lib/x86_64"
mkdir -p "${PACKAGE_DIR}/dex"

# Copy libraries
cp "${OUTPUT_DIR}/arm64-v8a/libviewit_plugin_office_universal.so" "${PACKAGE_DIR}/lib/arm64-v8a/"
cp "${OUTPUT_DIR}/x86_64/libviewit_plugin_office_universal.so" "${PACKAGE_DIR}/lib/x86_64/"

# Copy plugin.json
cp "${PLUGIN_DIR}/plugin.json" "${PACKAGE_DIR}/"

# Generate classes.dex (requires dx/d8 from Android SDK)
if command -v d8 &> /dev/null; then
    echo "Generating classes.dex..."
    cd "${PLUGIN_DIR}/src/main/java"
    d8 --output "${PACKAGE_DIR}/dex" --lib "${ANDROID_SDK_HOME}/platforms/android-36/android.jar" ai/viewit/plugins/officeuniversal/OfficeUniversalPlugin.kt
else
    echo "WARNING: d8 not found, skipping classes.dex generation"
fi

# Create ZIP packages (one per ABI, entries relative to the package root).
# Remove stale zips first: `zip -r` appends, so a prior package/ prefix would leak in.
PLUGIN_VERSION="$(python3 -c "import json;print(json.load(open('${PLUGIN_DIR}/plugin.json'))['version'])")"
for ABI in "${ABIS[@]}"; do
    rm -f "${OUTPUT_DIR}/office-universal-${PLUGIN_VERSION}-${ABI}.zip"
    (cd "${PACKAGE_DIR}" && zip -r "${OUTPUT_DIR}/office-universal-${PLUGIN_VERSION}-${ABI}.zip" "lib/${ABI}" "plugin.json" "dex")
done

echo "Build complete! Output in ${OUTPUT_DIR}"