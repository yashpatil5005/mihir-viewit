#!/usr/bin/env bash
# Quality gate script for ViewIt plugin format compatibility.
# Runs all checks that must pass before a release tag.
# This script is READ-ONLY - it does not modify any files.

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
LOG_DIR="${VIEWIT_GATE_LOG_DIR:-$ROOT/build/quality-gate}"
export TMPDIR="${VIEWIT_TMPDIR:-$ROOT/build/tmp}"
mkdir -p "$LOG_DIR" "$TMPDIR"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

_ts0=$(date +%s%N)
_ts=$_ts0
mark() { local now; now=$(date +%s%N); echo "  [$(awk -v a="$now" -v b="$_ts" 'BEGIN{printf "%.1fs", (a-b)/1e9}')] $1"; _ts=$now; }

FAILED=0
PASSED=0
SKIPPED=0

echo "=== ViewIt Quality Gate ==="
echo ""

# 0. Git worktree clean (check FIRST).
#    Releases still require a clean worktree, but local iteration can opt in
#    via VIEWIT_ALLOW_DIRTY=1 so cargo/frontend/plugin checks run without
#    forcing a commit first. CI runs unset this env var to keep the guard.
echo "[1/11] Git worktree clean..."
if [[ -z "$(git status --porcelain --untracked-files=normal)" ]]; then
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
mark "git worktree check"

# 1. Rust workspace library tests
echo "[2/11] Rust tests..."
if npm run test:rust >"$LOG_DIR/rust-tests.log" 2>&1; then
  echo -e "${GREEN}PASS${NC}  Rust tests"
  PASSED=$((PASSED+1))
else
  echo -e "${RED}FAIL${NC}  Rust tests (see $LOG_DIR/rust-tests.log)"
  FAILED=$((FAILED+1))
fi
mark "rust tests"

# 2. TypeScript tests
echo "[3/11] TypeScript tests..."
if npm run test:ts >"$LOG_DIR/ts-tests.log" 2>&1; then
  echo -e "${GREEN}PASS${NC}  TypeScript tests"
  PASSED=$((PASSED+1))
else
  echo -e "${RED}FAIL${NC}  TypeScript tests (see $LOG_DIR/ts-tests.log)"
  FAILED=$((FAILED+1))
fi
mark "ts tests"

echo "[4/11] Release tooling tests..."
if npm run test:release >"$LOG_DIR/release-tests.log" 2>&1; then
  echo -e "${GREEN}PASS${NC}  Release tooling tests"
  PASSED=$((PASSED+1))
else
  echo -e "${RED}FAIL${NC}  Release tooling tests (see $LOG_DIR/release-tests.log)"
  FAILED=$((FAILED+1))
fi
mark "release tooling tests"

# 3. Svelte typecheck
echo "[5/11] Svelte check..."
if (cd packages/ui && npx svelte-check --tsconfig ./tsconfig.json) >"$LOG_DIR/svelte-check.log" 2>&1; then
  echo -e "${GREEN}PASS${NC}  Svelte check (0 errors / 0 warnings)"
  PASSED=$((PASSED+1))
else
  echo -e "${RED}FAIL${NC}  Svelte check (see $LOG_DIR/svelte-check.log)"
  FAILED=$((FAILED+1))
fi
mark "svelte check"

# 4. Base app frontend build
echo "[6/11] Frontend build..."
if (cd apps/mobile && npm run build >"$LOG_DIR/frontend-build.log" 2>&1); then
  echo -e "${GREEN}PASS${NC}  Frontend build"
  PASSED=$((PASSED+1))
else
  echo -e "${RED}FAIL${NC}  Frontend build (see $LOG_DIR/frontend-build.log)"
  FAILED=$((FAILED+1))
fi
mark "frontend build"

# 2. Plugin crate tests
echo "[7/11] Plugin crate tests..."
if (cd plugins/office-ooxml && cargo test >"$LOG_DIR/plugin-tests.log" 2>&1); then
  echo -e "${GREEN}PASS${NC}  Plugin crate tests"
  PASSED=$((PASSED+1))
else
  echo -e "${RED}FAIL${NC}  Plugin crate tests (see $LOG_DIR/plugin-tests.log)"
  FAILED=$((FAILED+1))
fi
mark "plugin crate tests"

# 3. Plugin ZIP exists and is valid
echo "[8/11] Plugin ZIP build..."
plugin_version(){ python3 -c "import json;print(json.load(open('$1'))['version'])"; }
if [[ "${VIEWIT_PLUGIN_GATE_ONLY:-}" == "office-universal" ]]; then
  PLUGIN_ARTIFACTS=(
    "apps/mobile/plugins/office-universal/build/output/office-universal-$(plugin_version apps/mobile/plugins/office-universal/plugin.json)-arm64-v8a.zip"
    "apps/mobile/plugins/office-universal/build/output/office-universal-$(plugin_version apps/mobile/plugins/office-universal/plugin.json)-x86_64.zip"
  )
else
  PLUGIN_ARTIFACTS=(
    "apps/mobile/plugins/office-universal/build/output/office-universal-$(plugin_version apps/mobile/plugins/office-universal/plugin.json)-arm64-v8a.zip"
    "apps/mobile/plugins/compression-universal/build/output/compression-universal-$(plugin_version apps/mobile/plugins/compression-universal/plugin.json)-arm64-v8a.zip"
    "apps/mobile/plugins/font-universal/build/output/font-universal-$(plugin_version apps/mobile/plugins/font-universal/plugin.json)-arm64-v8a.zip"
    "apps/mobile/plugins/iwork-universal/build/output/iwork-universal-$(plugin_version apps/mobile/plugins/iwork-universal/plugin.json)-arm64-v8a.zip"
    plugins/pptx-vanilla-1.0.1.zip
    "plugins/player-base-$(plugin_version plugins/player-base/plugin.json).zip"
    plugins/editor-base-0.1.0.zip
  )
fi
MISSING_ARTIFACTS=()
for artifact in "${PLUGIN_ARTIFACTS[@]}"; do
  [[ -f "$artifact" ]] || MISSING_ARTIFACTS+=("$artifact")
done
if [[ ${#MISSING_ARTIFACTS[@]} -eq 0 ]]; then
  echo -e "${GREEN}PASS${NC}  Plugin ZIP build (${#PLUGIN_ARTIFACTS[@]} required artifacts)"
  PASSED=$((PASSED+1))
else
  echo -e "${RED}FAIL${NC}  Plugin ZIP build (run scripts/package-plugins.sh first)"
  printf '  missing: %s\n' "${MISSING_ARTIFACTS[@]}"
  FAILED=$((FAILED+1))
fi
mark "plugin zip check"

# 4. Catalog checksum matches ZIP
echo "[9/11] Catalog checksum verification..."
if [[ -f plugins/catalog.json ]]; then
  CATALOG_ARGS=(--catalog plugins/catalog.json
    --search-root apps/mobile/plugins --search-root plugins --search-root build)
  if [[ -n "${VIEWIT_PLUGIN_GATE_ONLY:-}" ]]; then
    CATALOG_ARGS+=(--require-id "$VIEWIT_PLUGIN_GATE_ONLY")
  else
    CATALOG_ARGS+=(--require-all)
  fi
  python3 scripts/verify_catalog_artifacts.py "${CATALOG_ARGS[@]}"
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
mark "catalog checksum"

# 5. Size budget
echo "[10/11] Size budget..."
if [[ -f scripts/size-budget.ts ]] && [[ -f scripts/size-budget.json ]]; then
  if (node --experimental-strip-types scripts/size-budget.ts 2>/dev/null || timeout 90 npx tsx scripts/size-budget.ts) >"$LOG_DIR/size-budget.log" 2>&1; then
    echo -e "${GREEN}PASS${NC}  Size budget"
    PASSED=$((PASSED+1))
  else
    echo -e "${RED}FAIL${NC}  Size budget (see $LOG_DIR/size-budget.log)"
    FAILED=$((FAILED+1))
  fi
else
  echo -e "${YELLOW}SKIP${NC}  Size budget (scripts not found)"
  SKIPPED=$((SKIPPED+1))
fi
mark "size budget"

# 6. Dependency separation: ooxmlsdk NOT in base Cargo.lock
echo "[11/11] Dependency separation (ooxmlsdk not in base)..."
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
mark "dependency separation"

echo "  [total: $(awk -v a="$(date +%s%N)" -v b="$_ts0" 'BEGIN{printf "%.1fs", (a-b)/1e9}')]"
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
  if bash "$ROOT/scripts/verify-android-16kb.sh" "$APK" >"$LOG_DIR/android-16kb.log" 2>&1; then
    echo -e "${GREEN}PASS${NC}  16 KB APK compatibility"
    PASSED=$((PASSED+1))
  else
    echo -e "${RED}FAIL${NC}  16 KB APK compatibility (see $LOG_DIR/android-16kb.log)"
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
