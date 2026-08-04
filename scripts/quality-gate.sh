#!/usr/bin/env bash
# Quality gate script for ViewIt plugin format compatibility.
# Runs all checks that must pass before a release tag.
# This script is READ-ONLY - it does not modify any files.

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

FAILED=0
PASSED=0
SKIPPED=0

echo "=== ViewIt Quality Gate ==="
echo ""

# 0. Git worktree clean (check FIRST).
#    Releases still require a clean worktree, but local iteration can opt in
#    via VIEWIT_ALLOW_DIRTY=1 so cargo/frontend/plugin checks run without
#    forcing a commit first. CI runs unset this env var to keep the guard.
echo "[1/7] Git worktree clean..."
if git diff --quiet && git diff --cached --quiet; then
  echo -e "${GREEN}PASS${NC}  Git worktree clean"
  PASSED=$((PASSED+1))
else
  if [[ "${VIEWIT_ALLOW_DIRTY:-0}" == "1" ]]; then
    echo -e "${YELLOW}SKIP${NC}  Git worktree dirty (VIEWIT_ALLOW_DIRTY=1 — release builds must unset this env)"
    SKIPPED=$((SKIPPED+1))
  else
    echo -e "${RED}FAIL${NC}  Git worktree has uncommitted changes"
    echo "  Commit or stash changes before running the release quality gate."
    echo "  For local iteration only: VIEWIT_ALLOW_DIRTY=1 scripts/quality-gate.sh"
    FAILED=$((FAILED+1))
    # Early exit - can't guarantee clean state for releases
    echo ""
    echo "=== Results ==="
    echo -e "Passed:  ${GREEN}0${NC}"
    echo -e "Failed:  ${RED}1${NC}"
    echo -e "Skipped: ${YELLOW}0${NC}"
    echo ""
    echo -e "${RED}QUALITY GATE FAILED${NC}"
    exit 1
  fi
fi

# 1. Base app frontend build
echo "[2/7] Frontend build..."
if (cd apps/mobile && npm run build >/tmp/viewit-build.log 2>&1); then
  echo -e "${GREEN}PASS${NC}  Frontend build"
  PASSED=$((PASSED+1))
else
  echo -e "${RED}FAIL${NC}  Frontend build (see /tmp/viewit-build.log)"
  FAILED=$((FAILED+1))
fi

# 2. Plugin crate tests
echo "[3/7] Plugin crate tests..."
if (cd plugins/office-ooxml && cargo test >/tmp/viewit-plugin-test.log 2>&1); then
  echo -e "${GREEN}PASS${NC}  Plugin crate tests"
  PASSED=$((PASSED+1))
else
  echo -e "${RED}FAIL${NC}  Plugin crate tests (see /tmp/viewit-plugin-test.log)"
  FAILED=$((FAILED+1))
fi

# 3. Plugin ZIP exists and is valid
echo "[4/7] Plugin ZIP build..."
PLUGIN_VERSION=$(python3 -c "import json; print(json.load(open('plugins/office-ooxml/plugin.json'))['version'])")
if [[ -f plugins/office-ooxml-$PLUGIN_VERSION-arm64-v8a.zip ]] && [[ -f plugins/office-ooxml-$PLUGIN_VERSION-x86_64.zip ]]; then
  echo -e "${GREEN}PASS${NC}  Plugin ZIP build"
  PASSED=$((PASSED+1))
else
  echo -e "${RED}FAIL${NC}  Plugin ZIP build (run scripts/prepare-release.sh first; ZIPs are ignored local artifacts)"
  FAILED=$((FAILED+1))
fi

# 4. Catalog checksum matches ZIP
echo "[5/7] Catalog checksum verification..."
if [[ -f plugins/catalog.json ]]; then
  python3 - <<'PY'
import hashlib, json, sys
from pathlib import Path
catalog = json.loads(Path("plugins/catalog.json").read_text())["plugins"]
zips = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in Path("plugins").glob("*.zip")}
failed = False
for e in catalog:
    checksum = (e.get("checksum") or "").strip().lower()
    if not checksum:
        continue
    related = [n for n in zips if n.startswith(e["id"] + "-")]
    if not related:
        print(f"  SKIP  {e['id']:<18} checksum {checksum[:12]}… (artifact not staged locally)")
        continue
    if checksum in zips.values():
        print(f"  OK    {e['id']:<18} checksum matches staged ZIP")
    else:
        print(f"  FAIL  {e['id']:<18} checksum {checksum[:12]}… does not match any staged ZIP")
        failed = True
sys.exit(1 if failed else 0)
PY
  CATALOG_CHECK=$?
  if [[ "$CATALOG_CHECK" == "0" ]]; then
    echo -e "${GREEN}PASS${NC}  Catalog checksum verification"
    PASSED=$((PASSED+1))
  else
    echo -e "${RED}FAIL${NC}  Catalog checksum mismatch"
    FAILED=$((FAILED+1))
  fi
else
  echo -e "${YELLOW}SKIP${NC}  Catalog checksum verification (catalog.json not found)"
  SKIPPED=$((SKIPPED+1))
fi

# 5. Size budget
echo "[6/7] Size budget..."
if [[ -f scripts/size-budget.ts ]] && [[ -f scripts/size-budget.json ]]; then
  if (node --experimental-strip-types scripts/size-budget.ts 2>/dev/null || npx tsx scripts/size-budget.ts) >/tmp/viewit-size.log 2>&1; then
    echo -e "${GREEN}PASS${NC}  Size budget"
    PASSED=$((PASSED+1))
  else
    echo -e "${RED}FAIL${NC}  Size budget (see /tmp/viewit-size.log)"
    FAILED=$((FAILED+1))
  fi
else
  echo -e "${YELLOW}SKIP${NC}  Size budget (scripts not found)"
  SKIPPED=$((SKIPPED+1))
fi

# 6. Dependency separation: ooxmlsdk NOT in base Cargo.lock
echo "[7/7] Dependency separation (ooxmlsdk not in base)..."
BASE_LOCK="apps/mobile/src-tauri/Cargo.lock"
ROOT_LOCK="Cargo.lock"
OOXMLSDK_IN_BASE=false
if [[ -f "$BASE_LOCK" ]] && grep -q "ooxmlsdk" "$BASE_LOCK"; then
  OOXMLSDK_IN_BASE=true
fi
if [[ -f "$ROOT_LOCK" ]] && grep -q "ooxmlsdk" "$ROOT_LOCK"; then
  if [[ -f plugins/office-ooxml/Cargo.lock ]] && grep -q "ooxmlsdk" plugins/office-ooxml/Cargo.lock; then
    if ! $OOXMLSDK_IN_BASE; then
      echo -e "${GREEN}PASS${NC}  Dependency separation (ooxmlsdk only in plugin)"
      PASSED=$((PASSED+1))
    else
      echo -e "${RED}FAIL${NC}  Dependency separation (ooxmlsdk in base Cargo.lock)"
      FAILED=$((FAILED+1))
    fi
  else
    if ! $OOXMLSDK_IN_BASE; then
      echo -e "${GREEN}PASS${NC}  Dependency separation (ooxmlsdk not in base)"
      PASSED=$((PASSED+1))
    else
      echo -e "${RED}FAIL${NC}  Dependency separation (ooxmlsdk in base Cargo.lock)"
      FAILED=$((FAILED+1))
    fi
  fi
else
  if ! $OOXMLSDK_IN_BASE; then
    echo -e "${GREEN}PASS${NC}  Dependency separation (ooxmlsdk not found anywhere)"
    PASSED=$((PASSED+1))
  else
    echo -e "${RED}FAIL${NC}  Dependency separation (ooxmlsdk in base Cargo.lock)"
    FAILED=$((FAILED+1))
  fi
fi

echo ""
echo "=== Results ==="
echo -e "Passed:  ${GREEN}$PASSED${NC}"
echo -e "Failed:  ${RED}$FAILED${NC}"
echo -e "Skipped: ${YELLOW}$SKIPPED${NC}"
echo ""

# Optional bonus gate: 16 KB compatibility of the most recent signed APK.
# This runs only when an APK already exists in dist/. CI trips this if a
# release build regresses the ELF alignment / zipalign page-size.
APK="$ROOT/dist/viewit-android-universal-debug.apk"
if [[ -f "$APK" && -x "$ROOT/scripts/verify-android-16kb.sh" ]]; then
  echo "[bonus] 16 KB APK compatibility..."
  if bash "$ROOT/scripts/verify-android-16kb.sh" "$APK" >/tmp/viewit-16kb.log 2>&1; then
    echo -e "${GREEN}PASS${NC}  16 KB APK compatibility"
    PASSED=$((PASSED+1))
  else
    echo -e "${RED}FAIL${NC}  16 KB APK compatibility (see /tmp/viewit-16kb.log)"
    FAILED=$((FAILED+1))
  fi
  echo ""
  echo "=== Results (incl. bonus) ==="
  echo -e "Passed:  ${GREEN}$PASSED${NC}"
  echo -e "Failed:  ${RED}$FAILED${NC}"
  echo -e "Skipped: ${YELLOW}$SKIPPED${NC}"
  echo ""
fi

if [[ $FAILED -gt 0 ]]; then
  echo -e "${RED}QUALITY GATE FAILED${NC}"
  exit 1
else
  echo -e "${GREEN}QUALITY GATE PASSED${NC}"
  exit 0
fi
