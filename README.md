# ViewIt

Universal offline file viewer — Tauri v2 (desktop + Android) + a web build,
with an on-device signed plugin catalog for format-specific parsers.

> **ViewIt opens files locally.** It supports a wide matrix of formats (text,
> images, PDF, office, audio/video, archives, ebooks, fonts) without sending
> your data anywhere. See [`docs/plan/`](docs/plan) and
> [`docs/STREAMING-ARCHITECTURE.md`](docs/STREAMING-ARCHITECTURE.md) for design.

## Monorepo layout

```
apps/            per-target frontends (Svelte 5)
  desktop/       Tauri v2 desktop (src-tauri)
  mobile/        Tauri v2 Android (src-tauri + bundled universal plugins)
  web/           static web build (WASM format parsers)
crates/          Rust workspace — format parsers (fmt-*), core, core-types
packages/        shared TS/Svelte
  platform/      single seam between UI and backend (Tauri IPC / WASM)
  ui/            shared viewers, theme, search
plugins/         distributable plugin catalog + signed catalog.signed.json
scripts/         build/smoke/dev automation (e.g. android-release.sh)
docs/            ADRs, plans, research, architecture
```

## Prerequisites

- Rust via [`rust-toolchain.toml`](rust-toolchain.toml) (pinned 1.96, incl.
  Android targets) + the stable Node version in [`.nvmrc`](.nvmrc).
- Tauri system deps for your desktop platform (see
  [tauri.app](https://tauri.app)); for Android, Android SDK/NDK on `ANDROID_HOME`.

```sh
npm install        # workspace deps (npm workspaces: packages/*, apps/*)
```

## Quickstart

```sh
npm run dev:web       # run the web app in the browser
npm run dev:desktop   # Tauri desktop (dev)
npm run build:web     # build web + WASM format parsers
bash scripts/android-release.sh   # build + install the Android APK
```

## Tasks

In addition to npm scripts above, a [`Makefile`](Makefile) wraps the most common
operations (`make test`, `make lint`, `make build-android`, `make deny`, …).

## Verification

- `make test` — Rust unit tests + `packages/platform` vitest units.
- `make lint` — prettier check across TS/Svelte/JS.
- `make check` — TypeScript typecheck of `packages/platform`.
- `make build-android` + on-device smoke —
  `python3 scripts/android-format-smoke.py` (see `scripts/`).
- CI runs these plus `cargo fmt --check`, `clippy -D warnings`, and reverifies
  the plugin catalog signature on every PR.

## License

MIT **or** Apache-2.0, at your option — see [LICENSE](LICENSE).
