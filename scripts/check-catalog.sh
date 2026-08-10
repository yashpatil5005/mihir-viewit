#!/usr/bin/env bash
# Health-check the deployed plugin catalog + every plugin zip.
#   scripts/check-catalog.sh [BASE] [--skip-downloads]
# BASE defaults to https://omnia.mihirpatil.co (the Pages proxy).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BASE="${1:-https://omnia.mihirpatil.co}"
SKIP_DOWNLOADS="${2:-}"

TMP="$(mktemp -d)"; trap 'rm -rf "$TMP"' EXIT
echo "[check] base: $BASE"
echo "[check] fetching catalog.signed.json"
curl -fsSL --max-time 20 "$BASE/catalog.signed.json" -o "$TMP/catalog.signed.json" || { echo "FAIL: cannot fetch catalog.signed.json"; exit 1; }

echo "[check] verifying Ed25519 signature"
if ! python3 "$ROOT/scripts/sign-catalog.py" verify --input "$TMP/catalog.signed.json"; then
  echo "FAIL: catalog signature invalid"; exit 1
fi
echo "[check] signature OK"

python3 - "$TMP/catalog.signed.json" "$BASE" "$SKIP_DOWNLOADS" <<'PY'
import hashlib, json, subprocess, sys, urllib.request
signed_path, base, skip = sys.argv[1], sys.argv[2], sys.argv[3]
d = json.load(open(signed_path))
plugins = d.get("catalog", {}).get("plugins", [])
print(f"[check] validating {len(plugins)} entries")
fails = 0
for p in plugins:
    url = p.get("downloadUrl", "")
    checksum = p.get("checksum", "")
    size = p.get("sizeBytes", -1)
    tag = f"{p.get('id')} v{p.get('version')} abi={p.get('abi') or '-'}"
    # expand relative downloadUrl against base
    if not url.lower().startswith("http"):
        url = base.rstrip("/") + "/" + url.lstrip("/")
    try:
        req = urllib.request.Request(url, method="GET", headers={"User-Agent": "viewit-catalog-check"})
        with urllib.request.urlopen(req, timeout=25) as r:
            code = r.status
            data = r.read()
        if code != 200:
            raise SystemExit  # fallthrough to fail
    except Exception as e:
        print(f"  FAIL  {tag}: cannot fetch ({e})"); fails += 1; continue
    if skip:
        print(f"  OK    {tag}: http {code}, {len(data)}B (checksum skipped)")
        continue
    if len(data) != size:
        print(f"  FAIL  {tag}: size {len(data)} != {size}"); fails += 1; continue
    if checksum and hashlib.sha256(data).hexdigest() != checksum.lower():
        print(f"  FAIL  {tag}: sha256 mismatch"); fails += 1; continue
    print(f"  OK    {tag}: http {code}, {len(data)}B, sha256 ok")
print("\n[check] " + ("ALL PLUGINS OK" if fails == 0 else f"{fails} FAILURES"))
sys.exit(1 if fails else 0)
PY