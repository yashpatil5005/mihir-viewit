#!/bin/bash
set -euo pipefail

PLUGIN_DIR="$(cd "$(dirname "$0")" && pwd)"
BUILD_DIR="$PLUGIN_DIR/build"
ANDROID_HOME="${ANDROID_HOME:-$HOME/Android/Sdk}"
BUILD_TOOLS="$ANDROID_HOME/build-tools/$(ls "$ANDROID_HOME/build-tools" | sort -V | tail -1)"
ANDROID_JAR="$ANDROID_HOME/platforms/android-36/android.jar"
JAVAC="$(which javac)"
NDK="${ANDROID_NDK_HOME:-${NDK_HOME:-$ANDROID_HOME/ndk/26.1.10909125}}"
export ANDROID_NDK_HOME="$NDK"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android24-clang"
export CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER="$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin/x86_64-linux-android24-clang"
export PATH="$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin:$PATH"

echo "=== Building Enhanced Office OOXML Plugin ==="
echo "Plugin dir: $PLUGIN_DIR"
echo "Build dir: $BUILD_DIR"

rm -rf "$BUILD_DIR"
mkdir -p "$BUILD_DIR/classes" "$BUILD_DIR/native/dex" "$BUILD_DIR/native/lib/arm64-v8a" "$BUILD_DIR/native/lib/x86_64" "$BUILD_DIR/packages"

cargo build \
    --manifest-path "$PLUGIN_DIR/Cargo.toml" \
    --target aarch64-linux-android \
    --release
TARGET_DIR="$(cargo metadata --manifest-path "$PLUGIN_DIR/Cargo.toml" --format-version 1 --no-deps | node -pe 'JSON.parse(require("fs").readFileSync(0, "utf8")).target_directory')"
cp "$TARGET_DIR/aarch64-linux-android/release/libviewit_plugin_office_ooxml.so" \
    "$BUILD_DIR/native/lib/arm64-v8a/"

cargo build \
    --manifest-path "$PLUGIN_DIR/Cargo.toml" \
    --target x86_64-linux-android \
    --release
cp "$TARGET_DIR/x86_64-linux-android/release/libviewit_plugin_office_ooxml.so" \
    "$BUILD_DIR/native/lib/x86_64/"

"$JAVAC" \
    -source 1.8 -target 1.8 \
    -classpath "$ANDROID_JAR" \
    -d "$BUILD_DIR/classes" \
    "$PLUGIN_DIR/src/Plugin.java" \
    "$PLUGIN_DIR/src/ViewItDocumentPlugin.java" \
    "$PLUGIN_DIR/src/PluginProgress.java"

"$BUILD_TOOLS/d8" \
    --min-api 24 \
    --output "$BUILD_DIR/native/dex/" \
    $(find "$BUILD_DIR/classes" -name "*.class" ! -path "*/ai/viewit/app/*")

cp "$PLUGIN_DIR/plugin.json" "$BUILD_DIR/native/plugin.json"

package_abi() {
    local abi="$1"
    local out="$BUILD_DIR/office-ooxml-0.1.0-$abi.zip"
    local pkg="$BUILD_DIR/packages/$abi"
    rm -rf "$pkg"
    mkdir -p "$pkg/dex" "$pkg/lib/$abi"
    cp "$BUILD_DIR/native/dex/classes.dex" "$pkg/dex/classes.dex"
    cp "$BUILD_DIR/native/lib/$abi/libviewit_plugin_office_ooxml.so" "$pkg/lib/$abi/libviewit_plugin_office_ooxml.so"
    cp "$PLUGIN_DIR/plugin.json" "$pkg/plugin.json"
    (cd "$pkg" && zip -r "$out" .)
    cp "$out" "$PLUGIN_DIR/../office-ooxml-0.1.0-$abi.zip"
}

package_abi arm64-v8a
package_abi x86_64

(cd "$BUILD_DIR/native" && zip -r "$BUILD_DIR/office-ooxml-0.1.0.zip" .)
cp "$BUILD_DIR/office-ooxml-0.1.0.zip" "$PLUGIN_DIR/../office-ooxml-0.1.0.zip"

echo ""
echo "=== Build Complete ==="
echo "Outputs:"
ls -lh "$BUILD_DIR"/office-ooxml-0.1.0*.zip
