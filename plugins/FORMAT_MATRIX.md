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

| Extension(s) | Detection | Base View | Enhanced | External | Fixtures | Limitations |
|---|---|---|---|---|---|---|
| `.docx`, `.docm` | extension + ZIP+XML | DocxViewer (text+structure) | `office-ooxml` plugin | — | `/tmp/opencode/viewit-fixtures/viewit-sample.docx` | No images in base; no footnotes/headers/tracked changes; enhanced: images+hyperlinks+headings+lists+tables |
| `.xlsx`, `.xlsm` | extension + ZIP+XML | XlsxViewer (text+structure) | `office-ooxml` plugin | — | `/tmp/opencode/viewit-fixtures/viewit-sample.xlsx` | No formatting/formulas in base; enhanced: merged cells+frozen panes |
| `.pptx`, `.pptm` | extension + ZIP+XML | PptxViewer (text extraction) | `office-ooxml` plugin | — | `/tmp/opencode/viewit-fixtures/viewit-sample.pptx` | No layout in base; enhanced: positioned text+images+bullets+rich runs+backgrounds |
| `.odt`, `.ods`, `.odp` | extension + ZIP+XML | PlaceholderViewer | planned | — | — | Fallback to text extraction |
| `.epub`, `.mobi`, `.azw3`, `.fb2` | extension | EpubViewer | planned | — | — | Basic reflow; no DRM |
| `.doc`, `.xls`, `.ppt` | extension + magic | UnsupportedViewer | planned (conversion) | open-with-external | — | Legacy binary OOXML; no native parser |
| `.rtf` | extension + magic | TextViewer | planned | — | — | Plain text fallback |
| `.heic`, `.heif`, `.avif` | extension + MIME | ImageViewer (platform) | planned | — | — | Depends on Android platform decoder |
| `.psd`, `.dng`, `.cr2`, `.cr3`, `.nef`, `.arw` | extension + MIME | ImageViewer (raw fallback) | planned | — | — | WebView may not render; platform-dependent |

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
| Archives | ✅ | ✅ | — | — |
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
