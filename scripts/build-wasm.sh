#!/usr/bin/env bash
# Build the WASM plugins used by the web target.
#
# Each fmt-* crate with a `wasm_entry.rs` is compiled with `wasm-pack` into its
# own `pkg/` directory (gitignored — generated artifact). The generator JS is
# consumed by `packages/platform/src/plugins/*Universal.ts`.
#
# Requires: wasm-pack, installed risc-v/wasm32 target.
set -euo pipefail
cd "$(dirname "$0")/.."

for crate in office archive ebook font iwork; do
  echo "==> wasm-pack viewit-fmt-${crate}"
  (cd "crates/fmt-${crate}" && wasm-pack build --target web --out-dir pkg --features wasm)
done

echo "Done. WASM pkg dirs regenerated."
