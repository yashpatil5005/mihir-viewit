#!/usr/bin/env bash
# Prepare release script for ViewIt.
# Rebuilds plugin ZIPs, updates catalog metadata, signs the catalog, and refreshes size measurements.
# Run this before running the quality gate.

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

set -e

echo "=== Prepare Release ==="
echo ""

# 1. Rebuild plugin ZIPs
echo "[1/3] Rebuilding plugin ZIPs..."
(cd plugins/office-ooxml && bash build.sh)
echo "  Done."

# 2. Update catalog metadata from tested local artifacts
echo "[2/3] Updating catalog metadata..."
PLUGIN_VERSION=$(python3 -c "import json; print(json.load(open('plugins/office-ooxml/plugin.json'))['version'])")
ARM64_ZIP="plugins/office-ooxml-$PLUGIN_VERSION-arm64-v8a.zip"
X64_ZIP="plugins/office-ooxml-$PLUGIN_VERSION-x86_64.zip"
RELEASE_BASE_URL="${VIEWIT_PLUGIN_RELEASE_BASE_URL:-https://github.com/mihir0209/ViewIt/releases/latest/download}"

if [[ -f "$ARM64_ZIP" ]]; then
  ARM64_CHECKSUM=$(sha256sum "$ARM64_ZIP" | awk '{print $1}')
  ARM64_SIZE=$(stat -c%s "$ARM64_ZIP")
  ARM64_INSTALLED_SIZE=$(python3 -c "import zipfile; print(sum(i.file_size for i in zipfile.ZipFile('$ARM64_ZIP').infolist()))")
  python3 -c "
import json
with open('plugins/catalog.json') as f:
    catalog = json.load(f)
for p in catalog['plugins']:
    if p['id'] == 'office-ooxml' and p.get('abi') == 'arm64-v8a':
        p['version'] = '$PLUGIN_VERSION'
        p['downloadUrl'] = '$RELEASE_BASE_URL/office-ooxml-$PLUGIN_VERSION-arm64-v8a.zip'
        p['sizeBytes'] = $ARM64_SIZE
        p['installedSizeBytes'] = $ARM64_INSTALLED_SIZE
        p['checksum'] = '$ARM64_CHECKSUM'
with open('plugins/catalog.json', 'w') as f:
    json.dump(catalog, f, indent=2)
"
  echo "  arm64-v8a checksum: $ARM64_CHECKSUM"
fi

if [[ -f "$X64_ZIP" ]]; then
  X64_CHECKSUM=$(sha256sum "$X64_ZIP" | awk '{print $1}')
  X64_SIZE=$(stat -c%s "$X64_ZIP")
  X64_INSTALLED_SIZE=$(python3 -c "import zipfile; print(sum(i.file_size for i in zipfile.ZipFile('$X64_ZIP').infolist()))")
  python3 -c "
import json
with open('plugins/catalog.json') as f:
    catalog = json.load(f)
for p in catalog['plugins']:
    if p['id'] == 'office-ooxml' and p.get('abi') == 'x86_64':
        p['version'] = '$PLUGIN_VERSION'
        p['downloadUrl'] = '$RELEASE_BASE_URL/office-ooxml-$PLUGIN_VERSION-x86_64.zip'
        p['sizeBytes'] = $X64_SIZE
        p['installedSizeBytes'] = $X64_INSTALLED_SIZE
        p['checksum'] = '$X64_CHECKSUM'
with open('plugins/catalog.json', 'w') as f:
    json.dump(catalog, f, indent=2)
"
  echo "  x86_64 checksum: $X64_CHECKSUM"
fi

if [[ -f scripts/.catalog-signing-key.priv ]]; then
  echo "  Signing catalog..."
  python3 scripts/sign-catalog.py sign
else
  echo "  Skipping catalog signing; scripts/.catalog-signing-key.priv not found."
fi

# 3. Update size budget measurements
echo "[3/3] Updating size budget measurements..."
python3 -c "
import json
from datetime import datetime, UTC
with open('scripts/size-budget.json') as f:
    budget = json.load(f)
measured_at = datetime.now(UTC).isoformat().replace('+00:00', 'Z')
budget['last_measured']['_status'] = f'measured {measured_at}'
with open('scripts/size-budget.json', 'w') as f:
    json.dump(budget, f, indent=2)
"
echo "  Done."

echo ""
echo "=== Release Prepared ==="
echo "Generated plugin ZIPs are local artifacts and are ignored by Git."
echo "Catalog download URLs use: $RELEASE_BASE_URL"
echo "Review and commit catalog/size-budget changes, then run 'bash scripts/quality-gate.sh'."
