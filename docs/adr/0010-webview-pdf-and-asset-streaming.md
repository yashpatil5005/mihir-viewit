# ADR 0010: WebView PDF + asset streaming (default mobile)

## Status

Accepted (supersedes ADR 0002 **as default Android PDF path**; pdfium remains optional).

## Context

- Bundled **libpdfium** adds ~6.1 MB per ABI ([android-usb-test.md](../android-usb-test.md)).
- Tauri **asset protocol** + `convertFileSrc` already supports **streaming** video/PDF URLs to the WebView without extra Rust crates ([Tauri core API](https://v2.tauri.app/reference/javascript/api/namespacecore/#convertfilesrc)).

## Decision

1. **Default Android** build profile **`fmt-everything-lite`**: no `fmt-pdf` / no `libpdfium.so`.
2. **PDF**: open via URI pass-through; render with **`<iframe>`** / **`<embed>`** using `convertFileSrc(uri)`; optional **pdf.js** lazy chunk if embed fails.
3. **Video/audio**: pass-through `Document::Media`; `<video>` / `<audio>` + `convertFileSrc`; no `read_uri_bytes` on open.
4. **Raster PDF** (pdfium): optional feature `fmt-pdf` for desktop or explicit “high fidelity” mobile builds only.

## Consequences

- Smaller APK; PDF fidelity varies by WebView version.
- Search-in-PDF may need pdf.js later (not pdfium text API).
- `android-release.sh` must support `lite` vs `full` profiles.