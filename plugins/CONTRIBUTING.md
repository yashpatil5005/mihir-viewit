# Contributing plugins to ViewIt

This documents the **two** plugin concepts and how to add a new one. Read the
schema references below before writing a descriptor.

## Two plugin concepts (do not confuse)

| | `plugins/` | `apps/mobile/plugins/` |
|---|---|---|
| Purpose | **Distributable catalog** — plugin source + published `.zip` + signed manifest | **Universal plugins bundled into the app** at build time |
| Contains | `ffmpeg-transcoder` (Java), `office-ooxml` (Rust + Java), `catalog.json` + `catalog.signed.json`, `.zip` artifacts | `compression-universal`, `font-universal`, `office-universal`, `pptx-vanilla` |
| Distributed via | Ed25519-signed `catalog.signed.json` | compiled into the APK |
| Architected for | download-on-demand from a signed catalog | always-present, no trust decision needed |

New work should generally be a **bundled universal plugin** (source under
`apps/mobile/plugins/<name>/`) unless it genuinely needs out-of-band delivery.

## Plugin source layout

Modern plugins keep Rust and Android code **side-by-side**, not nested:

```
apps/mobile/plugins/<name>/
├── plugin.json          # descriptor (schema below)
├── build.sh             # cross-compiles + zips the plugin
├── Cargo.toml           # (Rust plugins) cdylib, lib = "<name>"
└── src/
    ├── main/java/       # Kotlin/Java: Plugin entry (ViewItPlugin interface)
    └── main/rust/       # Rust native renderer (cdylib → .so)
```

For a Java-only plugin (e.g. `ffmpeg-transcoder`) omit `src/main/rust`.

## plugin.json schema

See `plugins/DISTRIBUTION_DESIGN.md` and `plugins/SCHEMA.md` for the full
canonical schema. The minimum useful descriptor:

```json
{
  "id": "<name>",
  "name": "Human Readable Name",
  "version": "0.1.0",
  "description": "What this plugin renders",
  "minAppVersion": 1,
  "entryClass": "ai.viewit.plugins.<name>.<Name>Plugin",
  "supportedFormats": ["png", "txt"],
  "capabilities": ["document-render"],
  "downloadUrl": "",
  "sizeBytes": 0,
  "installedSizeBytes": 0,
  "checksum": "",
  "abi": "arm64-v8a",
  "abiVersion": 1,
  "runtime": "native"
}
```

`entryClass` must match the package/class of the Kotlin entry, and the class
must implement the app's `ViewItPlugin` interface (see
`apps/mobile/src-tauri/src/.../PluginManager.kt`).

## Adding a plugin — checklist

1. Copy `plugins/_template/` to `apps/mobile/plugins/<name>/` (bundled) or
   `plugins/<name>/` (catalog-distributed).
2. Fill `plugin.json`; keep `supportedFormats` accurate — it drives routing.
3. Implement the entry class + (if native) the Rust renderer.
4. Add a `build.sh` mirroring `font-universal/build.sh`.
5. If catalog-distributed: **re-sign the catalog** (never run CI signing):
   ```sh
   python3 scripts/sign-catalog.py sign        # uses git-ignored private key
   ```
   CI verifies the signature (`.github/workflows/catalog-sign.yml`).
6. Extend `scripts/android-format-smoke.py` fixture set and run it on-device.

## Security

- The catalog **signature must verify** for any distributed plugin. The private
  key is never committed or used in CI.
- Do not add GPL/copyleft deps — see `deny.toml` (`make deny`).
- Keep parser work in the plugin's own crate/`.so` so a format parse bug cannot
  compromise the host.

See [`SECURITY.md`](../SECURITY.md).
