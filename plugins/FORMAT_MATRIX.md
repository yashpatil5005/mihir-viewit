# Format Compatibility Matrix

This document maps every format ViewIt handles to its detection method, rendering path, plugin availability, external fallback, test coverage, and known limitations.

## Legend

| Status | Meaning |
|---|---|
| `base` | Built-in viewer in the core app |
| `enhanced` | Optional downloadable plugin available |
| `external` | Handed off to another app or service |
| `unsupported` | No rendering path; user sees fallback |
| `planned` | On the roadmap but not yet implemented |

## Strong Base Formats

| Extension(s) | Detection | Base View | Enhanced | External | Fixtures | Limitations |
|---|---|---|---|---|---|---|
| `.txt` | extension + MIME | TextViewer | — | — | `/tmp/opencode/viewit-fixtures/` | Max 32 MB load; binary files rejected |
| `.md`, `.markdown` | extension | MarkdownViewer | — | — | — | GFM tables, no math |
| `.json`, `.jsonl` | extension + parse | JsonViewer | — | — | — | Prettified; large files slow |
| `.csv`, `.tsv` | extension | CsvViewer | — | — | — | First row as header; 200 row preview |
| `.png`, `.jpg`, `.jpeg`, `.webp`, `.gif`, `.bmp`, `.svg` | extension + MIME | ImageViewer | planned (RAW/PSD/HEIC) | — | — | SVG rendered by WebView; HEIC needs platform support |
| `.pdf` | extension + magic bytes | PdfViewer (WebView PDF) | planned (libpdfium full build) | — | — | Lite build: WebView native PDF; no annotations/forms |
| `.zip`, `.tar`, `.gz`, `.7z`, `.rar` | extension + magic | ArchiveViewer | — | — | — | Listing only; no in-archive editing |

## Enhanced Plugin Candidates

Status legend for current engine: **FULL** = real layout/content rendered on-device; **PARTIAL** = text/structure only; **GAP** = opens but does not render the format. Verified on-device 2026-08-05 (CDP DOM capture, `office-universal` + `pptx-vanilla`). Smoke run: 7/10 PASS; the 3 expected FAIL rows are the ODF-presentation (odp/otp) and legacy-ppt gaps below.

| Extension(s) | Detection | Base View | Enhanced | Notes / Limitations |
|---|---|---|---|---|
| `.docx`, `.docm` | extension + ZIP+XML | DocxViewer (text+structure) | `office-universal` (Enhanced Office OOXML + docx-preview) | **FULL** — images+hyperlinks+headings+lists+tables rendered |
| `.dotx`, `.dotm` | extension + ZIP+XML | DocxViewer | `office-universal` | **FULL** (template) — same OOXML text engine as docx |
| `.xlsx`, `.xlsm`, `.xlsb`, `.xls` | extension + magic | XlsxViewer (text+structure) | `office-universal` (calamine) | **FULL** — multi-sheet, formulas, 13/13 rows etc. on fixture |
| `.pptx`, `.pptm`, `.potx` | extension + ZIP+XML | PptxViewer (text extraction) | `pptx-vanilla` (JS, WebView HTML slides) | **FULL** slides — body shows "Rendered by PPTX Vanilla Viewer · view-only (no PowerPoint animations, no editing)"; no PowerPoint animations/editing |
| `.odt`, `.ott` | extension + ZIP+XML | PlaceholderViewer | `office-universal` | **FULL** — words/blocks/tables count on fixture (458 words, 33 blocks, 2 tables) |
| `.ods`, `.ots` | extension + ZIP+XML | PlaceholderViewer | `office-universal` | **FULL** — sheets, rows/cols, 24 formulas on fixture |
| `.odp`, `.otp` | extension + ZIP+XML | PlaceholderViewer | (routed to pptx-vanilla) | **GAP** — on-device body: "Invalid PPTX: presentation.xml not found" + "No slides". ODF presentation (content.xml) not parsed. Needs ODP path in office-universal |
| `.doc` | extension + magic | UnsupportedViewer/partial | `office-universal` (text extract) | **PARTIAL** — text-only preview; layout/images not preserved (known issue, see below) |
| `.ppt` | extension + magic | UnsupportedViewer | — | **GAP** — on-device body: "This presentation is password protected." Legacy binary slide content not rendered; message is misleading, should say unsupported + open-external |
| `.rtf` | extension + magic | TextViewer | — | Plain-text fallback — on-device smoke PASS (marker text extracted) |
| `.epub`, `.mobi`, `.azw3`, `.fb2` | extension | EpubViewer | — | Basic reflow; no DRM |
| `.heic`, `.heif`, `.avif` | extension + MIME | ImageViewer (platform) | — | Depends on Android platform decoder |
| `.psd`, `.dng`, `.cr2`, `.cr3`, `.nef`, `.arw` | extension + MIME | ImageViewer (raw fallback) | — | WebView may not render; platform-dependent |

### Compression Universal — 2026-08-05

`compression-universal` (native plugin, v0.1.0) lists **every** supported container precisely — zip, 7z, rar, tar, tar.gz/bz2/xz/zst/lz4/lzma chains, and standalone gz/bz2/xz/zst/lz4/lzma — by peeking at archive headers without extracting. Per-entry in-place preview, per-entry Save (system document picker), and "Extract all" to the app-scoped external storage folder.

| Extension(s) | Detection | Base View | Enhanced | Notes / Limitations |
|---|---|---|---|---|
| `.zip` | magic `PK` | ArchiveViewer (wasm) | `compression-universal` | **FULL** — precise entries incl. sizes (deflate/bzip2/zstd/lzma members) |
| `.7z` | magic `7z¼¯'` | UnsupportedViewer | `compression-universal` | **FULL** — entries incl. CRC + compressed sizes; encrypted (`AES256`) 7z surfaces the clear "encrypted archive" message instead of a listing |
| `.rar` | magic `Rar!` | UnsupportedViewer | `compression-universal` | **FULL** — entries via unrar; encrypted headers return an explicit password-required error |
| `.tar` | ustar magic | ArchiveViewer (wasm) | `compression-universal` | **FULL** |
| `.tar.gz`, `.tgz` | gzip magic + ustar | ArchiveViewer (gz→tar chain) | `compression-universal` | **FULL** — one member is listed as the inner tar |
| `.tar.bz2`, `.tbz2` | bzip2 magic | UnsupportedViewer | `compression-universal` | **FULL** |
| `.tar.xz`, `.txz` | xz magic | UnsupportedViewer | `compression-universal` | **FULL** |
| `.tar.zst`, `.tzst` | zstd magic | UnsupportedViewer | `compression-universal` | **FULL** |
| `.tar.lz4`, `.tlz` | lz4 frame magic | UnsupportedViewer | `compression-universal` | **FULL** |
| `.tar.lzma` | lzma-alone magic (`\x5d\x00\x00`) | UnsupportedViewer | `compression-universal` | **FULL** |
| `.gz`, `.bz2`, `.xz`, `.zst`, `.lz4`, `.lzma` (standalone) | per-format magic | UnsupportedViewer | `compression-universal` | **FULL** — single decompressed member; large members (≥ 64 MB uncompressed) fall back to Save |
| encrypted `.rar` | header flag | — | `compression-universal` | Encrypted-list error string surfaced ("password" message). No password prompt yet. |

Extraction storage scope (documented in catalog): listings read app-scoped temp copies of the picked file; extraction writes only to the app's own external storage (`files/Extracted/`) or, via the system document picker, to folders the user explicitly chooses — no storage permissions requested.

### .doc (legacy binary) decision — 2026-08-05

`doc` renders as a **text-only partial preview** (`44,544 bytes · encoding: utf-8 [Partial preview — Word .doc legacy binary, layout/formatting not preserved]`).

User-base context (research, 2026-08-05): `.doc` (binary CFB, Word 97‑2003) is a legacy-format — default replaced by `.docx` since Office 2007 (18 years). Word 2003-era users are <1%. Real-world `.doc` today is **archive + vertical-restricted**: legal/court e-filing requirements, healthcare EMR, government archives, K-12 legacy IT, and "that's what we've always used" orgs. Growth: negligible; new `.doc` creation is rare.

Decision (preview-tier, do NOT build a binary layout renderer):
1. Keep `doc` at text-preview tier. Improve the extractor to surface headings/paragraphs/tables where the binary stream allows.
2. Add a clear one-line banner + "Open with external app" action (Word / Google Docs / LibreOffice) for users who need true layout.
3. Reconsider only if app telemetry shows `.doc` open volume above a threshold (e.g. >5% of Office opens).

## External Runtime Candidates

| Extension(s) | Detection | Base View | Enhanced | External | Fixtures | Limitations |
|---|---|---|---|---|---|---|
| `.mp4`, `.webm`, `.mkv`, `.mov`, `.avi`, `.wmv`, `.flv`, `.mp3`, `.aac`, `.flac`, `.ogg`, `.wav` | extension + MIME | MediaViewer (platform) | planned (`media-ffmpeg`) | open-with-external | — | Platform decoder first; unsupported codecs need transcode |
| `.password-protected` (any) | inspection | UnsupportedViewer | planned | — | — | Password prompt not yet implemented |
| `.encrypted` (any) | inspection | UnsupportedViewer | — | — | — | No decryption path |

## Unsupported Formats

| Extension(s) | Detection | Fallback | Notes |
|---|---|---|---|
| `.exe`, `.dll`, `.so`, `.bin` | extension + MIME | UnsupportedViewer | Executable rejection |
| `.apk` | extension + MIME | UnsupportedViewer | App package rejection |
| `.js`, `.html`, `.css` | extension | TextViewer | Code viewing only; no execution |
| `.log` | extension | TextViewer | Large file warning |
| anything > 32 MB | size gate | UnsupportedViewer | "File too large" with external open suggestion |

## Test Coverage

| Format Family | Detection Test | Base Open Test | Enhanced Test | External Test |
|---|---|---|---|---|
| TXT/MD/JSON/CSV | ✅ | ✅ | — | — |
| Images | ✅ | ✅ | — | — |
| PDF | ✅ | ✅ | — | — |
| Archives | ✅ | ✅ | ✅ (compression-universal) | — |
| DOCX | ✅ | ✅ | ✅ (plugin) | — |
| XLSX | ✅ | ✅ | ✅ (plugin) | — |
| PPTX | ✅ | ✅ | ✅ (plugin) | — |
| Media | ✅ | ✅ | — | ✅ (open-with-external) |
| Unsupported | ✅ | — | — | ✅ (fallback) |

## How to Update

1. Add or modify a row in the appropriate section.
2. If adding a new format family, create a fixture under `/tmp/opencode/viewit-fixtures/` or document why none exists.
3. Update the Test Coverage table if test status changes.
4. If moving a format between tiers, update both old and new rows.
