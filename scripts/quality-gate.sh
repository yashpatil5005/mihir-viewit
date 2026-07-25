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

# 0. Git worktree clean (check FIRST)
echo "[1/7] Git worktree clean..."
if git diff --quiet && git diff --cached --quiet; then
  echo -e "${GREEN}PASS${NC}  Git worktree clean"
  PASSED=$((PASSED+1))
else
  echo -e "${RED}FAIL${NC}  Git worktree has uncommitted changes"
  echo "  Commit or stash changes before running quality gate."
  FAILED=$((FAILED+1))
  # Early exit - can't guarantee clean state
  echo ""
  echo "=== Results ==="
  echo -e "Passed:  ${GREEN}0${NC}"
  echo -e "Failed:  ${RED}1${NC}"
  echo -e "Skipped: ${YELLOW}0${NC}"
  echo ""
  echo -e "${RED}QUALITY GATE FAILED${NC}"
  exit 1
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
CATALOG_CHECK=true
if [[ -f plugins/catalog.json ]]; then
  while IFS= read -r checksum; do
    if [[ -n "$checksum" ]]; then
      found=false
      for zip_file in plugins/*.zip; do
        if [[ -f "$zip_file" ]]; then
          actual=$(sha256sum "$zip_file" | awk '{print $1}')
          if [[ "$actual" == "$checksum" ]]; then
            found=true
            break
          fi
        fi
      done
      if ! $found; then
        echo "  WARNING: catalog checksum $checksum not found in any ZIP"
        CATALOG_CHECK=false
      fi
    fi
  done < <(grep -o '"checksum": "[a-f0-9]*"' plugins/catalog.json | sed 's/"checksum": "//;s/"//')
  if $CATALOG_CHECK; then
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
  if npx tsx scripts/size-budget.ts >/tmp/viewit-size.log 2>&1; then
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

if [[ $FAILED -gt 0 ]]; then
  echo -e "${RED}QUALITY GATE FAILED${NC}"
  exit 1
else
  echo -e "${GREEN}QUALITY GATE PASSED${NC}"
  exit 0
fi
