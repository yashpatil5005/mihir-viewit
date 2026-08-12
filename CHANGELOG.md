# Changelog

All notable changes to ViewIt are tracked here, grouped by release. This file
follows [Keep a Changelog](https://keepachangelog.com/) and the project uses
semantic versioning (see `.git` history / tags for exact cut points).

## [0.2.0] - 2026-08-12

### Added
- Android **Play App Bundle (AAB)** build + upload-key signing
  (`android-release.sh` `:app:bundleArm64Release` + `jarsigner`); artifacts
  `dist/viewit-android-arm64-release.aab` (6,302,910 B) and
  `dist/viewit-android-arm64-release.apk` (13,287,831 B).
- Production **upload keystore** generation (`~/.android/viewit-upload.jks`,
  RSA 3072, alias `viewit-upload`, credentials in `~/.android/viewit-upload.env`).
- Release documentation: `docs/PUBLISH-TO-PLAY.md` (exact Play Console steps),
  AAB/upload-key sections folded into `docs/ANDROID-RELEASE-CHECKLIST.md`.
- GitHub release `v0.2.0` attaching the AAB + APK.

### Fixed
- AAB signing previously attempted with `apksigner` (APK-only); corrected to
  `jarsigner` SHA-256 Sun RSA (AABs are Zip/JAR based).

### Notes (0.2.0)
- App version `0.2.0` → Android `versionCode` 2000, `versionName` 0.2.0.
- APK v2/v3 signed, 16 KB aligned; catalog/plugin distribution unchanged
  (all 12 plugin entries resolve at `omnia.mihirpatil.co/plugins/`).

## [Unreleased]

### Fixed
- Media (audio/video) and PDF playback no longer depend on
  `MANAGE_EXTERNAL_STORAGE`: shared files are materialized into the app-private
  cache at open and streamed/read from there, so in-app playback survives when
  Android auto-revokes the permission.
- All stream-server error responses now carry `Access-Control-Allow-Origin`, so
  cross-origin `fetch(stream_url)` in the WebView surfaces the real HTTP status
  instead of an opaque `TypeError: Failed to fetch`.
- aiff/aif now decode to WAV for in-app playback; PDF renders via pdf.js.
- Repo hygiene: removed a tracked bytecode artifact and re-anchored the
  `**/src-tauri/gen/` gitignore rule.

### Added
- Root metadata: `README.md`, `LICENSE*`, `CONTRIBUTING.md`, `SECURITY.md`,
  `CHANGELOG.md`, `.env.example`.
- Developer tooling: `.editorconfig`, `.nvmrc`, `.node-version`, root
  `Makefile`, wired `lint`/`check`/`test` npm scripts.
- CI: `ci.yml` (cargo fmt/clippy/test; npm lint/check/build) + `catalog-sign.yml`
  (catalog signature reverify); supply-chain `deny.toml` + `dependabot.yml`.
- Tests: unit tests added to a non-office format crate; vitest unit coverage
  for `packages/platform` pure logic.
- Tests: Playwright E2E harness for `apps/web` (`npm run test:e2e`), with a
  chromium project serving the production build.
- `docs/architecture/` diagram; `plugins/_template/` + `plugins/CONTRIBUTING.md`.

## [0.1.0-scaffold]

Initial scaffold: Tauri v2 (desktop + Android) + Svelte 5 monorepo, core text/
image/office/archive/ebook/font parsing, on-device plugin catalog with Ed25519
signing, Android release + size-budget CI gates, and the on-device format smoke
harness.
