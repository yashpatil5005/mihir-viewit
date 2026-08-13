#!/usr/bin/env bash
# Package + sign the plugin catalog, stage it for Cloudflare Pages, and deploy.
#
# The catalog host (omnia.mihirpatil.co) proxies to a Cloudflare Pages project;
# publishing here makes the signed catalog + plugin zips reachable on real
# devices. Requires wrangler auth (or CLOUDFLARE_API_TOKEN) and a Pages project.
#
# Env:
#   CLOUDFLARE_PAGES_PROJECT  (default: viewit-plugin-catalog)
#   CLOUDFLARE_PAGES_BRANCH   (default: main)
#   WRANGLER                  (default: npx wrangler@4.120.0)
# Usage: publish-plugins.sh [--skip-package] [--stage-only] [--only PLUGIN_ID]
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PROJECT="${CLOUDFLARE_PAGES_PROJECT:-viewit-plugin-catalog}"
BRANCH="${CLOUDFLARE_PAGES_BRANCH:-main}"
WRANGLER="${WRANGLER:-npx wrangler@4.120.0}"
mkdir -p "$ROOT/build"
STAGE="$(mktemp -d "$ROOT/build/plugin-pages.XXXXXX")"
trap 'rm -rf "$STAGE"' EXIT
SKIP_PACKAGE=0
STAGE_ONLY=0
ONLY_PLUGIN=""

while [ "$#" -gt 0 ]; do
  case "$1" in
    --skip-package) SKIP_PACKAGE=1 ;;
    --stage-only) STAGE_ONLY=1 ;;
    --only) shift; ONLY_PLUGIN="${1:?--only requires a plugin ID}" ;;
    *) echo "usage: publish-plugins.sh [--skip-package] [--stage-only] [--only PLUGIN_ID]" >&2; exit 2 ;;
  esac
  shift
done

echo "[publish] 1/3 package + sign"
if [ "$SKIP_PACKAGE" -eq 0 ]; then
  if [ -n "$ONLY_PLUGIN" ]; then
    bash "$ROOT/scripts/package-plugins.sh" --plugin "$ONLY_PLUGIN" >/dev/null
  else
    bash "$ROOT/scripts/package-plugins.sh" >/dev/null
  fi
fi

echo "[publish] 2/3 stage to $STAGE"
mkdir -p "$STAGE/plugins"
CATALOG_ARGS=(--output "$STAGE/catalog.json")
if [ -n "$ONLY_PLUGIN" ]; then CATALOG_ARGS+=(--only "$ONLY_PLUGIN"); fi
python3 "$ROOT/scripts/update-catalog.py" "${CATALOG_ARGS[@]}"
python3 "$ROOT/scripts/sign-catalog.py" sign \
  --input "$STAGE/catalog.json" --output "$STAGE/catalog.signed.json"
python3 "$ROOT/scripts/sign-catalog.py" verify --input "$STAGE/catalog.signed.json"
# Copy every plugin zip referenced by the catalog into staging/plugins/.
python3 - <<PY
import hashlib, json, shutil, sys, urllib.request
from pathlib import Path
root = Path("$ROOT"); stage = Path("$STAGE/plugins")
cat = json.loads((Path("$STAGE")/"catalog.json").read_text())
for entry in cat["plugins"]:
    for url in [entry["downloadUrl"]]:
        name = url.rsplit("/", 1)[-1]
        # find the local zip by filename across the repo plugin build outputs
        found = None
        for cand in (root/"apps"/"mobile"/"plugins", root/"plugins", root/"build"):
            p = cand / name
            if p.exists(): found = p; break
        if found is None:
            for p in (root/"apps"/"mobile"/"plugins").glob("*/build/output/*.zip"):
                if p.name == name: found = p; break
        if found is None:
            for p in (root/"plugins").glob("*/build/*.zip"):
                if p.name == name: found = p; break
        destination = stage / name
        if found and hashlib.sha256(found.read_bytes()).hexdigest() == entry["checksum"]:
            shutil.copy2(found, destination)
        else:
            print("  [publish] fetching unchanged artifact:", entry["id"], name)
            request = urllib.request.Request(url, headers={"User-Agent": "viewit-release-stager"})
            with urllib.request.urlopen(request, timeout=60) as response:
                destination.write_bytes(response.read())
PY
python3 - "$STAGE/catalog.json" "$STAGE/plugins" <<'PY'
import hashlib, json, sys
from pathlib import Path
catalog = json.loads(Path(sys.argv[1]).read_text())
plugins = Path(sys.argv[2])
signed = json.loads(Path(sys.argv[1]).with_name("catalog.signed.json").read_text())
if signed.get("catalog") != catalog:
    raise SystemExit("staged signed/unsigned catalogs differ")
for entry in catalog["plugins"]:
    artifact = plugins / entry["downloadUrl"].rsplit("/", 1)[-1]
    data = artifact.read_bytes()
    if len(data) != entry["sizeBytes"] or hashlib.sha256(data).hexdigest() != entry["checksum"]:
        raise SystemExit(f"staged artifact mismatch: {artifact}")
print(f"[publish] staged and verified {len(catalog['plugins'])} catalog artifacts")
PY

echo "[publish] 3/3 deploy $STAGE -> Pages project '$PROJECT'"
if [ "$STAGE_ONLY" -eq 1 ]; then
  echo "[publish] stage-only complete; deployment skipped"
  exit 0
fi
python3 - "$ROOT" "$STAGE" <<'PY'
import sys
from pathlib import Path
root, stage = map(Path, sys.argv[1:])
sys.path.insert(0, str(root / "scripts"))
from release_io import atomic_write_text, persist_git_blob
for name in ("catalog.json", "catalog.signed.json"):
    target = root / "plugins" / name
    atomic_write_text(target, (stage / name).read_text())
    persist_git_blob(target)
PY
$WRANGLER pages deploy "$STAGE" --project-name "$PROJECT" --branch "$BRANCH" --commit-dirty=true

echo ""
echo "[publish] done. Check: $ROOT/scripts/check-catalog.sh https://omnia.mihirpatil.co"
