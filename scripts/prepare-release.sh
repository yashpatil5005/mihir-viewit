#!/usr/bin/env bash
# Prepare release script for ViewIt.
# Rebuilds plugin ZIPs, updates catalog checksums, and commits the changes.
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

# 2. Update catalog checksums
echo "[2/3] Updating catalog checksums..."
ARM64_ZIP="plugins/office-ooxml-0.1.0-arm64-v8a.zip"
X64_ZIP="plugins/office-ooxml-0.1.0-x86_64.zip"

if [[ -f "$ARM64_ZIP" ]]; then
  ARM64_CHECKSUM=$(sha256sum "$ARM64_ZIP" | awk '{print $1}')
  python3 -c "
import json
with open('plugins/catalog.json') as f:
    catalog = json.load(f)
for p in catalog['plugins']:
    if p['id'] == 'office-ooxml' and p.get('abi') == 'arm64-v8a':
        p['checksum'] = '$ARM64_CHECKSUM'
with open('plugins/catalog.json', 'w') as f:
    json.dump(catalog, f, indent=2)
"
  echo "  arm64-v8a checksum: $ARM64_CHECKSUM"
fi

if [[ -f "$X64_ZIP" ]]; then
  X64_CHECKSUM=$(sha256sum "$X64_ZIP" | awk '{print $1}')
  python3 -c "
import json
with open('plugins/catalog.json') as f:
    catalog = json.load(f)
for p in catalog['plugins']:
    if p['id'] == 'office-ooxml' and p.get('abi') == 'x86_64':
        p['checksum'] = '$X64_CHECKSUM'
with open('plugins/catalog.json', 'w') as f:
    json.dump(catalog, f, indent=2)
"
  echo "  x86_64 checksum: $X64_CHECKSUM"
fi

# 3. Update size budget measurements
echo "[3/3] Updating size budget measurements..."
python3 -c "
import json
from datetime import datetime
with open('scripts/size-budget.json') as f:
    budget = json.load(f)
budget['last_measured']['_status'] = f'measured {datetime.utcnow().isoformat()}Z'
with open('scripts/size-budget.json', 'w') as f:
    json.dump(budget, f, indent=2)
"
echo "  Done."

# Commit the changes
echo ""
echo "Committing changes..."
git add plugins/office-ooxml-0.1.0-arm64-v8a.zip plugins/office-ooxml-0.1.0-x86_64.zip plugins/office-ooxml-0.1.0.zip plugins/catalog.json scripts/size-budget.json
git commit -m "chore: prepare release — rebuild plugin ZIPs, update checksums and size budget"

echo ""
echo "=== Release Prepared ==="
echo "Run 'bash scripts/quality-gate.sh' to verify."
