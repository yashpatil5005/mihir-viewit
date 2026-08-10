#!/usr/bin/env bash
# Package all Android feature plugins as DOWNLOADABLE catalog zips (nothing is
# baked into the APK). Each plugin's Java entry class is compiled with the
# office-ooxml recipe: javac (against ai.viewit.app stubs + android.jar) -> d8
# -> classes.dex, then staged per-ABI: plugin.json + dex/classes.dex +
# lib/<abi>/<so> [+ index.js]. ai/viewit/app/* is excluded from the dex (the
# host app provides those at runtime).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
D8="$(ls "$ANDROID_HOME"/build-tools/*/d8 2>/dev/null | sort -V | tail -1)"
[ -n "$D8" ] || D8="$HOME/Android/Sdk/build-tools/34.0.0/d8"
ANDROID_JAR="$HOME/Android/Sdk/platforms/android-36/android.jar"
STUBS="$ROOT/build/plugin-stubs"

write_stubs() {
  mkdir -p "$STUBS/ai/viewit/app"
  cat > "$STUBS/ai/viewit/app/ViewItDocumentPlugin.java" <<'EOF'
package ai.viewit.app;
import android.content.Context; import android.net.Uri; import java.util.List;
public interface ViewItDocumentPlugin {
    String getId(); String getVersion(); List<String> getSupportedFormats();
    void initialize(Context context); boolean canHandle(String mimeType);
    boolean canHandleExt(String ext); String render(Uri inputUri, String ext); void cleanup();
}
EOF
  cat > "$STUBS/ai/viewit/app/PluginProgress.java" <<'EOF'
package ai.viewit.app;
public class PluginProgress {
    public int progress; public String stage; public String message;
}
EOF
  cat > "$STUBS/ai/viewit/app/ArchivePlugin.java" <<'EOF'
package ai.viewit.app;
import android.net.Uri; import java.io.File;
public interface ArchivePlugin {
    String listArchive(Uri uri, String name);
    byte[] extractEntry(Uri uri, String name, String entryName);
    String extractEntryToFile(Uri uri, String name, String entryName, File outFile);
    String extractAll(Uri uri, String name, File destDir);
    String detectFormat(Uri uri, String name);
}
EOF
  echo "[package] stubs -> $STUBS"
}

normalize_tree() { python3 -c "
import os
from pathlib import Path
for e in sorted(Path('$1').rglob('*')): os.utime(e,(315532800,315532800))
os.utime('$1',(315532800,315532800))"; }

compile_dex() { # name, srcDir
  local name="$1" srcDir="$2"
  rm -rf "$STUBS/$name"-cls "$STUBS/$name"-dex
  mkdir -p "$STUBS/$name"-cls "$STUBS/$name"-dex
  javac -source 1.8 -target 1.8 -classpath "$STUBS:$ANDROID_JAR" -d "$STUBS/$name"-cls \
    "$STUBS/ai/viewit/app/ViewItDocumentPlugin.java" "$STUBS/ai/viewit/app/PluginProgress.java" \
    "$STUBS/ai/viewit/app/ArchivePlugin.java" $srcDir/*.java 2>&1 | grep -iE "error" && return 1
  "$D8" --min-api 24 --output "$STUBS/$name"-dex \
    $(find "$STUBS/$name"-cls -name '*.class' ! -path '*/ai/viewit/app/*') 2>&1 | grep -iE "error" && return 1
  echo "[package] $name: dex $(stat -c %s "$STUBS/$name"-dex/classes.dex) bytes"
}

package_plugin() { # pid, name, srcDir, sobase, with_js
  local pid="$1" name="$2" srcDir="$3" sobase="$4" with_js="$5"
  local PLUGIN_DIR="$ROOT/apps/mobile/plugins/$pid" version pkg out
  version="$(python3 -c "import json;print(json.load(open('$PLUGIN_DIR/plugin.json'))['version'])")"
  compile_dex "$name" "$srcDir"
  for abi in arm64-v8a x86_64; do
    local so="$PLUGIN_DIR/build/output/$abi/lib$sobase.so"
    [ -f "$so" ] || { echo "  [package] WARN missing $so"; continue; }
    pkg="$PLUGIN_DIR/build/output/package/$abi"; out="$PLUGIN_DIR/build/output/$pid-$version-$abi.zip"
    rm -rf "$pkg" "$out"; mkdir -p "$pkg/dex" "$pkg/lib/$abi"
    cp "$STUBS/$name"-dex/classes.dex "$pkg/dex/classes.dex"
    cp "$so" "$pkg/lib/$abi/lib$sobase.so"
    cp "$PLUGIN_DIR/plugin.json" "$pkg/plugin.json"
    if [ "$with_js" = "1" ]; then cp "$ROOT/build/plugins/office-universal/index.js" "$pkg/index.js"; fi
    normalize_tree "$pkg"
    (cd "$pkg" && zip -q -X -r "$out" .)
    echo "  [package] $pid $abi -> $(basename "$out") ($(du -h "$out" | cut -f1))"
  done
}

write_stubs
mkdir -p "$ROOT/build/plugins/office-universal"

# ---- JS bundles first (office-universal hybrid needs them) ----
echo "[package] building pptx-vanilla / office-universal JS bundles"
if [ ! -f "$ROOT/plugins/pptx-vanilla/dist/index.js" ]; then
  (cd "$ROOT/plugins/pptx-vanilla" && npm run build >/dev/null 2>&1)
fi
# office-universal bundle exposes the loader's expected global for that id.
(cd "$ROOT/plugins/pptx-vanilla" && node_modules/.bin/esbuild src/entry.js \
  --bundle --format=iife --global-name=ViewItPlugin__office_universal --minify \
  --external:three --external:three/ --outfile="$ROOT/build/plugins/office-universal/index.js" >/dev/null 2>&1)

# office-universal + font-universal + compression-universal natives.
package_plugin office-universal officeuniversal \
  "$ROOT/apps/mobile/plugins/office-universal/src/main/java/ai/viewit/plugins/officeuniversal" \
  viewit_plugin_office_universal 1
package_plugin font-universal fontuniversal \
  "$ROOT/apps/mobile/plugins/font-universal/src/main/java/ai/viewit/plugins/fontuniversal" \
  viewit_plugin_font_universal 0
package_plugin compression-universal compressionuniversal \
  "$ROOT/apps/mobile/plugins/compression-universal/src/main/java/ai/viewit/plugins/compressionuniversal" \
  viewit_plugin_compression_universal 0

# Standalone pptx-vanilla zip (js, per its plugin.json).
PVV="$ROOT/plugins/pptx-vanilla"; rm -rf "$PVV/zipout" && mkdir -p "$PVV/zipout"
cp "$PVV/dist/index.js" "$PVV/zipout/index.js"; cp "$PVV/plugin.json" "$PVV/zipout/plugin.json"
normalize_tree "$PVV/zipout"
(cd "$PVV/zipout" && zip -q -X -r "$ROOT/plugins/pptx-vanilla-1.0.1.zip" .)
echo "[package] pptx-vanilla zip: $(du -h "$ROOT/plugins/pptx-vanilla-1.0.1.zip" | cut -f1)"
