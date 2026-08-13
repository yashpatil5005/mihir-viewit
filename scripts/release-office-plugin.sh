#!/usr/bin/env bash
# Build, validate, optionally install, and optionally publish office-universal.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PLUGIN="$ROOT/apps/mobile/plugins/office-universal"
DEPLOY=0
INSTALL=0
SKIP_APK=0
ALLOW_DIRTY=0

usage() {
  echo "usage: release-office-plugin.sh [--install] [--deploy] [--skip-apk] [--allow-dirty]"
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --install) INSTALL=1 ;;
    --deploy) DEPLOY=1 ;;
    --skip-apk) SKIP_APK=1 ;;
    --allow-dirty) ALLOW_DIRTY=1 ;;
    -h|--help) usage; exit 0 ;;
    *) usage >&2; exit 2 ;;
  esac
  shift
done

cd "$ROOT"
if [ "$ALLOW_DIRTY" -eq 0 ] && [ -n "$(git status --porcelain)" ]; then
  echo "[release] worktree must be clean (or pass --allow-dirty)" >&2
  exit 2
fi

command -v cargo >/dev/null || { echo "[release] missing cargo" >&2; exit 2; }
command -v python3 >/dev/null || { echo "[release] missing python3" >&2; exit 2; }

MANIFEST_VERSION="$(python3 -c "import json; print(json.load(open('$PLUGIN/plugin.json'))['version'])")"
CARGO_VERSION="$(python3 -c "import tomllib; print(tomllib.load(open('$PLUGIN/Cargo.toml','rb'))['package']['version'])")"
if [ "$MANIFEST_VERSION" != "$CARGO_VERSION" ]; then
  echo "[release] version drift: plugin.json=$MANIFEST_VERSION Cargo.toml=$CARGO_VERSION" >&2
  exit 2
fi
grep -qE 'native[[:space:]]+String[[:space:]]+getVersion\(\)' \
  "$PLUGIN/src/main/java/ai/viewit/plugins/officeuniversal/OfficeUniversalPlugin.java" \
  || { echo "[release] Java wrapper must use native getVersion()" >&2; exit 2; }

echo "[release] 1/6 test office parser"
cargo test -p viewit-fmt-office

echo "[release] 2/6 build + package office-universal v$MANIFEST_VERSION"
bash "$PLUGIN/build.sh"
for abi in arm64-v8a x86_64; do
  python3 scripts/validate-plugin-package.py \
    "$PLUGIN/build/output/office-universal-$MANIFEST_VERSION-$abi.zip" \
    --id office-universal --version "$MANIFEST_VERSION" --abi "$abi"
done
python3 - "$PLUGIN" "$MANIFEST_VERSION" <<'PY'
import hashlib, json, sys
from pathlib import Path
plugin, version = Path(sys.argv[1]), sys.argv[2]
catalog = json.loads(Path("plugins/catalog.json").read_text())["plugins"]
for abi in ("arm64-v8a", "x86_64"):
    artifact = plugin / "build" / "output" / f"office-universal-{version}-{abi}.zip"
    checksum = hashlib.sha256(artifact.read_bytes()).hexdigest()
    published = next((entry for entry in catalog if entry.get("id") == "office-universal" and entry.get("version") == version and entry.get("abi") == abi), None)
    if published and published.get("checksum") != checksum:
        raise SystemExit(
            f"[release] office-universal v{version} {abi} already exists with different bytes; bump the plugin version before release"
        )
PY

if [ "$SKIP_APK" -eq 0 ]; then
  echo "[release] 3/6 build Android release APK"
  bash scripts/android-release.sh
else
  echo "[release] 3/6 skip Android APK"
fi

echo "[release] 4/6 update + sign local catalog"
bash scripts/publish-plugins.sh --skip-package --stage-only --only office-universal

echo "[release] 5/6 quality gate"
VIEWIT_ALLOW_DIRTY=1 VIEWIT_PLUGIN_GATE_ONLY=office-universal bash scripts/quality-gate.sh

if [ "$INSTALL" -eq 1 ]; then
  [ "$SKIP_APK" -eq 0 ] || { echo "[release] --install requires an APK build" >&2; exit 2; }
  command -v adb >/dev/null || { echo "[release] missing adb" >&2; exit 2; }
  adb get-state >/dev/null
  adb install -r dist/viewit-android-arm64-release.apk
  adb push "$PLUGIN/build/output/office-universal-$MANIFEST_VERSION-arm64-v8a.zip" \
    "/sdcard/Download/office-universal-$MANIFEST_VERSION.zip"
  echo "[release] installed APK and pushed plugin ZIP to device Downloads"
fi

if [ "$DEPLOY" -eq 1 ]; then
  echo "[release] 6/6 publish + verify"
  bash scripts/publish-plugins.sh --skip-package --only office-universal
  bash scripts/check-catalog.sh "${VIEWIT_CATALOG_BASE_URL:-https://omnia.mihirpatil.co}"
else
  echo "[release] 6/6 deploy skipped; pass --deploy to publish"
fi

echo "[release] office-universal v$MANIFEST_VERSION ready"
echo "[release] review and commit plugins/catalog*.json and scripts/size-budget.json"
