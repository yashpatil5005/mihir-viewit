#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

for command in node npm cargo dpkg-deb ldd xwininfo xprop xwd ffmpeg tesseract strace; do
  command -v "$command" >/dev/null || {
    echo "[desktop-e2e] missing required command: $command" >&2
    exit 2
  }
done

[[ -n "${DISPLAY:-}" ]] || {
  echo "[desktop-e2e] DISPLAY is not set; run inside an X11/XWayland desktop session" >&2
  exit 2
}

ARCH="$(uname -m)"
[[ "$ARCH" == "x86_64" ]] || {
  echo "[desktop-e2e] this Linux bundle profile currently targets x86_64, got $ARCH" >&2
  exit 2
}

echo "[desktop-e2e] generating deterministic local fixtures"
node scripts/generate-desktop-fixtures.mjs

echo "[desktop-e2e] building offline Linux .deb"
npm run tauri --workspace apps/desktop -- build --bundles deb

TARGET_DIR="$(cargo metadata --no-deps --format-version 1 | node -e '
  let input = "";
  process.stdin.on("data", chunk => input += chunk);
  process.stdin.on("end", () => process.stdout.write(JSON.parse(input).target_directory));
')"
DEBS=("$TARGET_DIR"/release/bundle/deb/ViewIt_*_amd64.deb)
[[ ${#DEBS[@]} -eq 1 && -f "${DEBS[0]}" ]] || {
  echo "[desktop-e2e] expected exactly one amd64 ViewIt .deb in $TARGET_DIR/release/bundle/deb" >&2
  exit 1
}
DEB="${DEBS[0]}"

PACKAGE_ROOT="$ROOT/build/desktop-package-root"
rm -rf "$PACKAGE_ROOT"
mkdir -p "$PACKAGE_ROOT"
dpkg-deb -x "$DEB" "$PACKAGE_ROOT"
BINARY="$PACKAGE_ROOT/usr/bin/viewit-desktop"
[[ -x "$BINARY" ]] || {
  echo "[desktop-e2e] packaged executable missing: $BINARY" >&2
  exit 1
}

if ldd "$BINARY" | grep -q "not found"; then
  echo "[desktop-e2e] packaged executable has unresolved shared libraries:" >&2
  ldd "$BINARY" | grep "not found" >&2
  exit 1
fi

echo "[desktop-e2e] verifying the packaged ELF makes no external network calls"
NETWORK_LOG="$ROOT/build/desktop-offline-network.log"
rm -f "$NETWORK_LOG"
strace -f -e trace=network -o "$NETWORK_LOG" \
  python3 scripts/test-desktop-offline-visual.py \
    --binary "$BINARY" \
    --fixtures "$ROOT/build/desktop-fixtures" \
    --output "$ROOT/build/desktop-offline-visual"

if grep -E 'connect\([^,]+, \{sa_family=(AF_INET|AF_INET6)' "$NETWORK_LOG" | grep -vE '127\.0\.0\.1|::1'; then
  echo "[desktop-e2e] external network access detected" >&2
  exit 1
fi

echo "[desktop-e2e] PASS: packaged native Linux app rendered every fixture offline"
echo "[desktop-e2e] bundle: $DEB"
echo "[desktop-e2e] report: $ROOT/build/desktop-offline-visual/report.json"
