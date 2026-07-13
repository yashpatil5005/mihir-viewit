#!/usr/bin/env bash
# pdfium-render 0.9.2: copy_from_nonoverlapping wants *const u8 on Rust 1.96+.
set -euo pipefail
REGISTRY="${CARGO_HOME:-$HOME/.cargo}/registry/src"
FILE=$(find "$REGISTRY" -path '*/pdfium-render-0.9.2/src/pdf/font/provider.rs' 2>/dev/null | head -1)
if [[ -z "$FILE" ]]; then
  echo "[patch-pdfium] pdfium-render not in cargo registry yet; run cargo fetch -p pdfium-render first"
  exit 0
fi
CHANGED=0
if grep -q 'chars.as_ptr() as \*const i8' "$FILE"; then
  sed -i 's/chars.as_ptr() as \*const u8/chars.as_ptr() as *const i8/' "$FILE"
  CHANGED=1
fi

if [[ $CHANGED -eq 1 ]]; then
  echo "[patch-pdfium] patched $FILE"
else
  echo "[patch-pdfium] already patched or upstream fixed"
fi