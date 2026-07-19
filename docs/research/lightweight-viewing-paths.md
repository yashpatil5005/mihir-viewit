# Research: lightweight viewing (streaming + WebView decoders)

Sources are primary: [Tauri v2 docs](https://v2.tauri.app), [docs.rs `tauri::Builder::register_uri_scheme_protocol`](https://docs.rs/tauri/2.0.0/tauri/struct.Builder.html#method.register_uri_scheme_protocol).

## 1. Built-in asset protocol (no extra Rust deps for file→WebView)

- Tauri ships **`asset:`** custom protocol when `app.security.assetProtocol.enable` is true ([asset protocol scope](https://v2.tauri.app/security/asset-protocol/)).
- Frontend: **`convertFileSrc(path)`** → URL the WebView can load ([core API](https://v2.tauri.app/reference/javascript/api/namespacecore/#convertfilesrc)).
- Official video example uses `<video><source src={convertFileSrc(filePath)}>` — **no full-file IPC**; the WebView reads via protocol/streaming.
- CSP must allow `asset:` / `http://asset.localhost` in `img-src` / `media-src` ([same doc](https://v2.tauri.app/reference/javascript/api/namespacecore/#convertfilesrc)).
- Scope: narrow globs (`$HOME/**/*`, persisted dialog paths via [persisted-scope `protocol-asset`](https://v2.tauri.app/plugin/persisted-scope/)) — not `open_bytes` for large media.

**ViewIt gap:** `tauri.conf.json` had `assetProtocol` disabled; platform never called `convertFileSrc` for share/`open_uri` paths (only blob URLs for picker images).

## 2. Custom URI scheme (optional, still zero npm)

- `Builder::register_uri_scheme_protocol` / `register_asynchronous_uri_scheme_protocol` — serve bytes from Rust with `http::Request` / `Response` ([docs.rs](https://docs.rs/tauri/2.0.0/tauri/struct.Builder.html#method.register_uri_scheme_protocol)).
- Use when asset scope cannot cover `content://` on Android; prefer **fs plugin + convertFileSrc** first (already in app).

## 3. PDF: pdfium (~6.1 MB APK) vs WebView/JS

| Approach | APK/native cost | Offline | Android WebView |
|----------|-----------------|---------|-----------------|
| Bundled **libpdfium** + raster PNG over IPC | ~6 MB `.so` + heavy IPC | Yes | N/A (images in UI) |
| **`<iframe>` / `<embed>`** + `convertFileSrc(uri)` | **0** extra native | Yes | Chromium WebView often renders PDF inline (device-dependent) |
| **pdf.js** (dynamic import, no Rust) | ~0 native, +~1–2 MB JS gzip | Yes if bundled | Consistent fallback |

**Conclusion:** Mobile default profile should be **`fmt-pdf` off** + WebView/pdf.js path; keep pdfium as **desktop** or optional `fmt-pdf-raster` feature.

## 4. Video / audio

- Same as images: **reject pre-read** was correct for RAM; **wrong** for product goal — should **stream** via `convertFileSrc` + `<video>` / `<audio>`, not `Unsupported`.
- `probe_uri` / `open_uri` must **not** call `read_uri_bytes` for media extensions.
- 32 MB cap applies to **parse-in-Rust** formats only, not pass-through media.

## 5. Android WebView

- System WebView = Chromium, updatable ([webview versions](https://v2.tauri.app/reference/webview-versions/)); PDF/video codec support follows that version, not app bundle.

## 6. Format coverage at minimum size

| Format | Strategy |
|--------|----------|
| Text/code/md/json/csv | Rust `fmt-text` (small) |
| Images | WebView `<img>` + `convertFileSrc` / blob |
| PDF | WebView embed + optional pdf.js chunk |
| Video/audio | `<video>`/`<audio>` + `convertFileSrc` |
| Office/archives | Existing Rust parsers (feature-gated) |