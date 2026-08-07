# Plugin template

Copy this directory to `apps/mobile/plugins/<name>/` (bundled universal) or
`plugins/<name>/` (catalog) and adapt. See `../CONTRIBUTING.md` for the full
policy, schema, and build steps.

```
<name>/
├── plugin.json       # descriptor (edit: id, name, entryClass, supportedFormats)
├── build.sh          # copy from font-universal/build.sh and rename artifacts
└── src/
    └── main/
        ├── java/     # ai/viewit/plugins/<name>/<Name>Plugin.kt (implements ViewItPlugin)
        └── rust/     # optional cdylib renderer: Cargo.toml + src/lib.rs
```

## Minimal descriptor

See `plugin.json.example` in this directory — fill `id`, `name`, `entryClass`,
`supportedFormats`, and `abi`.

## Quick steps

1. `cp -r plugins/_template apps/mobile/plugins/<name>`
2. Edit `plugin.json` (unique `id`, real `entryClass`).
3. Implement the Kotlin entry + (optionally) the Rust renderer.
4. Add `build.sh` (mirror `font-universal/build.sh`).
5. If catalog-distributed: `python3 scripts/sign-catalog.py sign`.
6. Add a fixture to `scripts/android-format-smoke.py` and run it on-device.
