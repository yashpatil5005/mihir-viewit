#!/bin/bash
set -euo pipefail

PLUGIN_DIR="$(cd "$(dirname "$0")" && pwd)"
BUILD_DIR="$PLUGIN_DIR/build"
FFMPEGKIT_VERSION="6.1.4"
FFMPEGKIT_URL="https://repo1.maven.org/maven2/com/mrljdx/ffmpeg-kit-full/$FFMPEGKIT_VERSION/ffmpeg-kit-full-$FFMPEGKIT_VERSION.aar"
SMART_EXCEPTION_VERSION="0.2.1"
SMART_EXCEPTION_URL="https://repo1.maven.org/maven2/com/mrljdx/smart-exception-java/$SMART_EXCEPTION_VERSION/smart-exception-java-$SMART_EXCEPTION_VERSION.jar"
ANDROID_HOME="${ANDROID_HOME:-$HOME/Android/Sdk}"
BUILD_TOOLS="$ANDROID_HOME/build-tools/$(ls "$ANDROID_HOME/build-tools" | sort -V | tail -1)"
ANDROID_JAR="$ANDROID_HOME/platforms/android-36/android.jar"

JAVAC="$(which javac)"
echo "=== Building FFmpeg Transcoder Plugin ==="
echo "Plugin dir: $PLUGIN_DIR"
echo "Build dir: $BUILD_DIR"
echo "javac: $JAVAC"
echo "Build tools: $BUILD_TOOLS"
echo "Android JAR: $ANDROID_JAR"

rm -rf "$BUILD_DIR"
mkdir -p "$BUILD_DIR"/{aar,native/dex,classes}

# 1. Download FFmpegKit AAR
echo "--- Downloading FFmpegKit AAR ---"
curl -L -o "$BUILD_DIR/aar/ffmpeg-kit.aar" "$FFMPEGKIT_URL"
curl -L -o "$BUILD_DIR/aar/smart-exception-java.jar" "$SMART_EXCEPTION_URL"

# 2. Extract native .so files
echo "--- Extracting native libraries ---"
cd "$BUILD_DIR/aar"
unzip -qo ffmpeg-kit.aar -d extracted
cp -r extracted/jni/* "$BUILD_DIR/native/"
rm -rf "$BUILD_DIR/native/armeabi-v7a" "$BUILD_DIR/native/x86" "$BUILD_DIR/native/x86_64"

# 3. Extract FFmpegKit classes (for DEX)
echo "--- Extracting FFmpegKit classes ---"
if [ -f extracted/classes.jar ]; then
    cd "$BUILD_DIR/classes"
    jar xf ../aar/extracted/classes.jar
fi
jar xf "$BUILD_DIR/aar/smart-exception-java.jar"

# 4. Compile plugin Java source
echo "--- Compiling plugin source ---"
mkdir -p "$BUILD_DIR/plugin_classes"

# Find all FFmpegKit classes we need
FFMPEGKIT_CP=""
if [ -f "$BUILD_DIR/aar/extracted/classes.jar" ]; then
    FFMPEGKIT_CP="$BUILD_DIR/aar/extracted/classes.jar:$BUILD_DIR/aar/smart-exception-java.jar"
fi

"$JAVAC" \
    -source 1.8 -target 1.8 \
    -classpath "$ANDROID_JAR:$FFMPEGKIT_CP" \
    -d "$BUILD_DIR/plugin_classes" \
    "$PLUGIN_DIR/src/Plugin.java" \
    "$PLUGIN_DIR/src/ViewItPlugin.java" \
    "$PLUGIN_DIR/src/PluginProgress.java"

# 5. Create DEX
echo "--- Creating DEX ---"
"$BUILD_TOOLS/d8" \
    --min-api 24 \
    --output "$BUILD_DIR/native/dex/" \
    $(find "$BUILD_DIR/plugin_classes" -name "*.class" ! -path "*/ai/viewit/app/*") \
    $(find "$BUILD_DIR/classes" -name "*.class" 2>/dev/null)

# 6. Package plugin ZIP
echo "--- Packaging plugin ZIP ---"
cd "$BUILD_DIR/native"
cp "$PLUGIN_DIR/plugin.json" .
zip -r "$PLUGIN_DIR/build/ffmpeg-transcoder-1.0.0.zip" .

echo ""
echo "=== Build Complete ==="
echo "Output: $PLUGIN_DIR/build/ffmpeg-transcoder-1.0.0.zip"
ls -lh "$PLUGIN_DIR/build/ffmpeg-transcoder-1.0.0.zip"
