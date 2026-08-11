# Plugin Distribution & Ops

How the optional, downloadable ViewIt plugins reach devices, and how to publish,
monitor, and version them.

## Topology

- **Source of truth**: `plugins/catalog.json` + per-plugin zips built by
  `scripts/package-plugins.sh` (native Java→dex zips, js bundles, per-ABI).
- **Signing**: `scripts/sign-catalog.py` signs `catalog.json` → `catalog.signed.json`
  (`{"catalog":..., "signature":...}` over a canonical JSON; `ensure_ascii=False` so
  the Android app's `canonicalJson` verifies it).
- **Host**: Cloudflare Pages project **`viewit-plugin-catalog-temp`**; the custom
  domain **`omnia.mihirpatil.co`** proxies to it. The app's release catalog URL is
  `https://omnia.mihirpatil.co/catalog.signed.json`.
- **App verification**: `CATALOG_PUBLIC_KEY_B64` in `PluginManager.kt` must match the
  signer's public key (`scripts/.catalog-signing-key.pub`). Catalog entries that fail
  verification are dropped (default catalog shows nothing → users had to add a URL).

## Release flow (one command)

```bash
CLOUDFLARE_PAGES_PROJECT=viewit-plugin-catalog-temp bash scripts/publish-plugins.sh
# package → sign → stage build/plugin-pages → wrangler pages deploy
bash scripts/check-catalog.sh https://omnia.mihirpatil.co   # health check
```

Manual equivalent:
```bash
ANDROID_HOME="$HOME/Android/Sdk" bash scripts/package-plugins.sh
python3 scripts/update-catalog.py && python3 scripts/sign-catalog.py sign
npx wrangler pages deploy build/plugin-pages --project-name viewit-plugin-catalog-temp --branch main
```

## Adding a plugin

1. Put it under `plugins/<id>/` (native Rust+Java or a js bundle) with `plugin.json`
   declaring `id`, `version`, `runtime`, `base` (view|play|edit|tool), `supportedFormats`,
   `capabilities`, `downloadUrl`.
2. Add a generator in `scripts/update-catalog.py` (see `pptx_vanilla_entries`,
   `player_base_entries`, `editor_base_entries`) that finds the staged zip and emits
   the entry (checksum/size/per-ABI).
3. `bash scripts/package-plugins.sh` (or build the js zip) → `python3 scripts/update-catalog.py`
   → `python3 scripts/sign-catalog.py sign` → publish + health check.

## Versioning

- A changed zip **must** change `version` in `plugin.json`, or re-installs short-circuit.
- Native-plugin **upgrades** on a running app are staged for the next cold start
  (`installZipPayload` → "Update downloaded. Restart the app to apply it."); the Plugin
  Store shows a **Restart to apply** button (bridge `restartApp()`).
- Same-artifact reinstalls reuse the in-process loaded instance (Android can't `dlclose`).

## Caching / stale content

- omnia (Cloudflare edge) caches `.zip` (public, max-age up to ~4 h) and can cache a **404**
  for a path that didn't exist before a deploy. After a republish, newly-added or changed
  paths may briefly serve stale/404 until the edge revalidates (seconds to minutes), or
  you purge the zone's cache. `check-catalog.sh` fetches with `cache: no-store` on the
  catalog and verifies each zip's sha256, so it reports the real state.

## Keys

- Keep `scripts/.catalog-signing-key.{priv,pub}` safe; the app ships the pub key. Regenerate
  with `python3 scripts/sign-catalog.py generate` only if you also update the app's key.
