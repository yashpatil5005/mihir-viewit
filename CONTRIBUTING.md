# Contributing to ViewIt

Thanks for helping. This describes how to work in the monorepo, how changes are
gated, and the plugin/catalog policy. Read it before your first PR.

## 1. Repo layout & tooling

- **Rust workspace** (root `Cargo.toml`): format parsers live in `crates/fmt-*`.
  Desktop + Android backends are `apps/*/src-tauri`. Toolchain is pinned in
  `rust-toolchain.toml`.
- **Frontends**: Svelte 5 + Vite in `apps/*`, sharing code in `packages/`.
  `packages/platform` is the **only** place that touches backend IPC/WASM; UI
  in `packages/ui` must consume it through that seam.
- **Node/npm workspaces**: always add deps to the nearest `package.json`; the
  lockfile is the source of truth — commit it.

## 2. Dev loop

```sh
npm install
npm run dev:web        # fastest loop
make lint test check   # the gates CI runs
make build-android     # full APK + install
python3 scripts/android-format-smoke.py   # on-device format smoke
```

## 3. Standards

- **Rust:** `cargo fmt`, `cargo clippy -D warnings`, and a `cargo test` that
  can run on a host without Android SDK (only `apps/*/src-tauri` needs NDK).
  Follow the per-crate `#[cfg(test)]` convention; add fixture bytes inline or
  under `crates/<name>/tests/fixtures/`.
- **TS/Svelte:** run `npm run format` (prettier + prettier-plugin-svelte).
  `npm run lint` is the CI gate. Keep `packages/platform` dependency-free of
  the UI; pure logic should stay testable (vitest).
- **Licensing:** the project is dual-licensed MIT **or** Apache-2.0. New
  dependencies must be compatible (see `deny.toml` allowlist). Mark BSD-3-Clause
  crates — they are compatible with MIT/Apache but not Apache-only.

## 4. Plugins & the signed catalog

There are **two plugin concepts**:

- `plugins/` — the **distributable catalog**: plugin source + published
  `.zip` artifacts + `catalog.json` **and** its Ed25519 signature
  `catalog.signed.json`. Anything here that changes the catalog **must** be
  re-signed and must not break verification.
- `apps/mobile/plugins/` — **universal plugins bundled into the app** at
  build time (compression, font, office, pptx-vanilla).

Changing catalog content (`plugins/catalog.json`) or any plugin descriptor
requires re-signing:

```sh
# uses scripts/.catalog-signing-key.priv (git-ignored; never commit keys)
python3 scripts/sign-catalog.py
```

CI reverifies `catalog.signed.json` on every PR (see `.github/workflows/
catalog-sign.yml`). See [`plugins/CONTRIBUTING.md`](plugins/CONTRIBUTING.md)
and [`plugins/DISTRIBUTION_DESIGN.md`](plugins/DISTRIBUTION_DESIGN.md).

## 5. PR checklist

- [ ] `make lint` and `make check` pass.
- [ ] `make test` passes (Rust + vitest).
- [ ] New/updated `fmt-*` crate includes or extends `#[cfg(test)]` coverage.
- [ ] If the plugin catalog changed, it is re-signed and CI verification passes.
- [ ] `npm run format` applied; diff is minimal and readable.
