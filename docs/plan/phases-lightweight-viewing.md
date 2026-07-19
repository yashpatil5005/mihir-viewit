# Plan: phases — lightweight frequent formats

Budget target: drop **~6 MB libpdfium** from default Android; avoid loading **video/PDF** into RAM.

## Phase A — Asset protocol + platform URLs (this sprint)

- Enable `assetProtocol` + CSP `media-src`/`frame-src` for mobile/desktop.
- `packages/platform`: `mediaUrlForUri(uri)` → `convertFileSrc` on Tauri.
- `Viewer`: pass `docUri` into media/PDF viewers.

**Done when:** shared PDF/video URI opens without `open_bytes`.

## Phase B — Pass-through open path (Rust)

- `core`: video/audio extensions → `Format::Media`; PDF → `Pdf` with `native: true`, empty `pages`.
- `uri_util`: `stream_extensions` — `open_from_uri` returns pass-through **without** `read_uri_bytes`.
- Remove video hard-reject from `probe_uri` (optional: keep audio as stream too).

**Done when:** `adb` share 50 MB `.mp4` opens player instantly.

## Phase C — UI viewers

- `MediaViewer.svelte` (`<video>` / `<audio>`).
- `PdfViewer`: if `native` → `<iframe src={assetUrl}>`; else existing raster + `pdfPage`.
- Lazy `import('pdfjs-dist')` fallback if iframe errors (Phase D).

## Phase D — Android build profile `fmt-everything-lite`

- Feature set: all parsers **except** `fmt-pdf`.
- `android-release.sh`: `BUILD_PROFILE=lite` skips pdfium download + JNI copy.
- ADR 0010 supersedes ADR 0002 default for Android.

## Phase E — Frequent formats matrix

- [x] text/code (extended sniff)
- [x] images WebView
- [ ] PDF native
- [ ] video/audio stream
- [x] office/archive (existing)
- [ ] size-check script updated

## Phase F — Web WASM (unchanged)

- Same pass-through rules in `platform` web path.