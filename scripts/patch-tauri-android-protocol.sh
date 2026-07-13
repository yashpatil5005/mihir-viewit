#!/usr/bin/env bash
set -euo pipefail
REGISTRY="${CARGO_HOME:-$HOME/.cargo}/registry/src"
FILE=$(find "$REGISTRY" -path '*/tauri-2.11.5/src/protocol/tauri.rs' 2>/dev/null | head -1)
if [[ -z "$FILE" ]]; then
  echo "[patch-tauri] tauri 2.11.5 not in registry; skip"
  exit 0
fi
if grep -q 'viewit-android-tauri-localhost-strip' "$FILE"; then
  echo "[patch-tauri] already patched"
  exit 0
fi
perl -i -0pe 's/  let path = path\n    \.strip_prefix\("tauri:\/\/localhost"\)\n    \.map\(\|p\| p\.to_string\(\)\)\n    \/\/ the `strip_prefix` only returns None when a request is made to `https:\/\/tauri\.\$P` on Windows and Android\n    \/\/ where `\$P` is not `localhost\/\*`\n    \.unwrap_or_default\(\);/  \/\/ viewit-android-tauri-localhost-strip\n  let path = path\n    .strip_prefix("tauri:\/\/localhost")\n    .or_else(|| path.strip_prefix("http:\/\/tauri.localhost"))\n    .or_else(|| path.strip_prefix("https:\/\/tauri.localhost"))\n    .map(|p| p.to_string())\n    .unwrap_or_else(|| {\n      if path.contains("tauri.localhost") {\n        path.split_once("tauri.localhost").map(|(_, rest)| rest.to_string()).unwrap_or(path)\n      } else {\n        path\n      }\n    });/s' "$FILE"
echo "[patch-tauri] patched $FILE"