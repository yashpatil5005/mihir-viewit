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
#   WRANGLER                  (default: npx wrangler@4.120.0, pinned — latest prereleases may reference unpublished Miniflare alphas)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PROJECT="${CLOUDFLARE_PAGES_PROJECT:-viewit-plugin-catalog}"
BRANCH="${CLOUDFLARE_PAGES_BRANCH:-main}"
WRANGLER="${WRANGLER:-npx wrangler@4.120.0}"
STAGE="$ROOT/build/plugin-pages"

echo "[publish] 1/3 package + sign"
bash "$ROOT/scripts/package-plugins.sh" >/dev/null
python3 "$ROOT/scripts/update-catalog.py"
python3 "$ROOT/scripts/sign-catalog.py" sign

echo "[publish] 2/3 stage to $STAGE"
rm -rf "$STAGE" && mkdir -p "$STAGE/plugins"
cp "$ROOT/plugins/catalog.json" "$STAGE/catalog.json"
cp "$ROOT/plugins/catalog.signed.json" "$STAGE/catalog.signed.json"
# Copy every plugin zip referenced by the catalog into staging/plugins/.
python3 - <<PY
import json, shutil
from pathlib import Path
root = Path("$ROOT"); stage = Path("$STAGE/plugins")
cat = json.loads((root/"plugins"/"catalog.json").read_text())
for entry in cat["plugins"]:
    for url in [entry["downloadUrl"]]:
        name = url.rsplit("/", 1)[-1]
        # find the local zip by filename across the repo plugin build outputs
        found = None
        for cand in (root/"apps"/"mobile"/"plugins", root/"plugins", root/"build"):
            p = cand / name
            if p.exists(): found = p; break
        if found is None:
            # last-resort: search common output paths
            for p in (root/"apps"/"mobile"/"plugins").glob("*/build/output/*.zip"):
                if p.name == name: found = p; break
        if found: shutil.copy2(found, stage / name)
        else: print("  [publish] WARN missing zip entry:", entry["id"], name)
PY
ls -1 "$STAGE/" "$STAGE/plugins/" | head -40

echo "[publish] 3/3 deploy $STAGE -> Pages project '$PROJECT'"
$WRANGLER pages deploy "$STAGE" --project-name "$PROJECT" --branch "$BRANCH"

echo ""
echo "[publish] done. Check: $ROOT/scripts/check-catalog.sh https://omia.mihirpatil.co"
