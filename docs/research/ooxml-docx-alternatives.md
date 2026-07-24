# Research: OOXML DOCX Parsing/Rendering Alternatives

Date: 2026-07-24

Scope: DOCX first, with XLSX/PPTX implications for ViewIt. Sources are primary only: crates.io, docs.rs, GitHub repositories, npm package pages, and project README/API docs from those sources.

## ViewIt Context

ViewIt's current Rust office path is `crates/fmt-office`. It maps `.docx` to `Document::Docx { blocks, byte_len }`, where `DocxBlock` is one of `Paragraph { text, heading }`, `ListItem { text, level }`, `Table { rows }`, or `Image { name }` (`crates/core-types/src/lib.rs`). The current parser uses `docx-rust` first and falls back to `zip` + `quick-xml` over `word/document.xml` when `docx-rust` parse fails (`crates/fmt-office/src/lib.rs`). XLSX/ODS use `calamine`; PPTX uses a custom ZIP/XML reader.

## Short Conclusion

For the existing `Document::Docx` model, the least disruptive options are:

1. Keep `docx-rust` plus the current `zip`/`quick-xml` fallback for a small Rust/Android-safe structured preview.
2. Evaluate `ooxmlsdk` as the best Rust replacement if we want deeper, relationship-aware OOXML package/schema access across DOCX/XLSX/PPTX, accepting pre-1.0 generated APIs and a larger schema surface.
3. Use `docx-preview` only if product direction shifts from structured preview to in-WebView DOCX rendering.
4. Use `mammoth` if semantic DOCX-to-HTML is preferred over layout fidelity; sanitize output before injecting into the UI.
5. Do not treat JSZip or `@ooxml-tools/file` as DOCX parsers/renderers; they are useful package primitives.

## Comparison Matrix

| Alternative | Formats | Parses/renders vs package/schema access | Browser/WebView viability | Rust/Android viability | Dependency footprint | License | Maturity/activity | Maps to `Document::Docx`? |
|---|---|---|---|---|---|---|---|---|
| Rust `ooxmlsdk` | DOCX, XLSX, PPTX | Reads, edits, writes, round-trips OOXML packages; generated schema types and strongly typed package parts. Not a visual renderer. | Not directly JS; possible only via WASM work, with Rust 2024/MSRV constraints. | Pure Rust, but current README states MSRV 1.88 for core crates and Rust 2024 workspace, which may not fit current ViewIt toolchain without upgrade. | docs.rs lists `quick-xml`, `zip` optional, `ooxmlsdk-derive`, `thiserror`, numeric/string helpers, optional `base64`, `bytes`, `regex`; large generated schema surface. | MIT OR Apache-2.0 | crates.io latest 0.12.0, published 2026-07-20; GitHub shows 947 commits, 68 stars, 11 forks; README says pre-1.0 and APIs can change. | Yes, but with custom traversal over typed WordprocessingML roots. Stronger for relationships/images/styles than fallback, more work than `docx-rust`. |
| Current `docx-rust` | DOCX | Parses and generates DOCX; exposes `DocxFile::from_reader`/`parse`, `BodyContent::{Paragraph, Table, ...}`, `Paragraph::text`. Not a renderer. | Rust crate; not directly browser unless compiled to WASM. Similar project note points to a writer with Rust/WASM, but this crate itself is not a JS renderer. | Already in ViewIt. Pure Rust crate using `zip` and `hard-xml`; current code compiles for Tauri/mobile profile if dependency graph does. | docs.rs lists `derive_more`, `hard-xml`, `log`, `thiserror`, `zip ^4`, optional `async_zip`/`futures-io`. | MIT | crates.io latest 0.1.11, published 2026-01-22; 2.1M total downloads and 618k recent downloads; docs.rs says 16.56% documented. | Yes. Already maps paragraphs, headings, lists, tables into `DocxBlock`. Limited for images/relationships/layout. |
| Current fallback: `zip` + `quick-xml` | Any OOXML ZIP in principle; current DOCX path reads `word/document.xml`; custom PPTX path reads presentation XML. | Package/XML streaming access only. It parses selected tags; no full schema validation, style cascade, relationships, numbering resolution, or rendering. | Rust-side only in current app. A JS equivalent would need JSZip plus XML parsing. | Already Android-friendly because it uses existing Rust deps and avoids large renderers. | `zip = 8.6.0`; `quick-xml = 0.36` in ViewIt. docs.rs for `quick-xml 0.36.2` lists only required normal dep `memchr`; docs.rs for `zip 8.6.0` lists `crc32fast`, `indexmap`, `memchr`, `typed-path`, plus many optional compression/encryption deps. | MIT for both `quick-xml 0.36.2` and `zip 8.6.0` per docs.rs. | Mature building blocks, but the fallback parser is ViewIt-specific and intentionally narrow. | Yes for current minimal blocks. Best control over output shape, weakest OOXML completeness. |
| JSZip | ZIP container, not OOXML-specific | Creates, reads, edits ZIP files. Does not parse DOCX XML semantics or render documents. | Good. npm/README examples use Blob, browser scripts, `generateAsync({type:"blob"})`; `@ooxml-tools/file` and `docx-preview` both build on JSZip. | JS dependency only; not useful inside Rust/Android parser except as WebView code. | npm page says 4 dependencies, 7,149 dependents, 31.7M weekly downloads; version 3.10.1 last published 4 years ago. | MIT OR GPL-3.0-or-later | Very mature/popular: GitHub 10.4k stars, 1.3k forks, 738 commits; last npm publish 4 years ago. | Indirect only. Could feed XML parser in WebView, but no block model by itself. |
| `@ooxml-tools/file` / github.com/ooxml-tools/file | DOCX and XLSX read/write; PPTX marked coming soon | OOXML package file helper layered on JSZip. README shows `open("docx", zip)`, `list`, CLI `unpack`, `pack`, `read`, `write`. Not semantic parser or renderer. | Explicitly supports nodejs/browser. | JS-only; not a Rust/Android parser unless used in WebView. | npm page says 4 dependencies, 2 dependents, 31 weekly downloads. Requires caller-provided JSZip instance. | MIT | Very young/small: npm 0.6.5 published a month ago; GitHub 154 commits, 4 stars, 0 forks. | Indirect only. Could simplify OOXML package file access before custom parsing, but still requires a DOCX semantic mapper. |
| `docx-preview` / docxjs | DOCX | Renders/converts DOCX into HTML. Public stable API is `renderAsync`; `parseAsync` and `renderDocument` are marked experimental/internal. Uses JSZip. | Strong. API accepts `Blob`, `ArrayBuffer`, or `Uint8Array` and renders to DOM elements; README shows browser usage. | JS/WebView path only. Not suitable for Rust model unless accepting HTML/DOM output instead of `Document::Docx`. | npm page says 1 dependency and 269 dependents; README notes library uses JSZip. | Apache-2.0 | Active/popular: npm 0.4.0 published 16 days ago; ~976k weekly downloads; GitHub 2.0k stars, 271 forks, 207 commits. README warns inner parsing/rendering may change and only `renderAsync` is stable. | Poor for current block model if used as a renderer. Possible but not ideal via experimental parse API; better as an alternate WebView renderer mode. |
| `mammoth` | DOCX | Converts DOCX to semantic HTML or raw text; supports headings, lists, custom style maps, tables, footnotes/endnotes, images, inline formatting, links, text boxes, comments. Does not aim for exact layout. | Strong. README says Node/browser supported; browser input supports `{arrayBuffer}` and standalone `mammoth.browser.js`. | JS path only. Other first-party ports exist for Python, Java/JVM, .NET, but no Rust crate from this project. | npm page says 10 dependencies, 1,353 dependents, 5.2M weekly downloads. | BSD-2-Clause | Mature/popular: npm 1.12.0 published 4 months ago; GitHub 6.3k stars, 661 forks, 1,143 commits. README warns no sanitization and possible pathological performance on crafted documents. | Medium. HTML output can be post-processed into blocks, but mapping from semantic HTML back to `DocxBlock` loses OOXML specifics. Raw text is too weak for tables/images. |
| Rust `calamine` | XLSX, XLSM/XLAM via `Xlsx`, XLS, XLSB, ODS | Spreadsheet reader/deserializer. Reads worksheet ranges, formulas, VBA project, defined names; optional picture support. Not DOCX/PPTX and not renderer. | Rust crate; not browser without WASM work. | Already in ViewIt for XLSX/ODS. Pure Rust and current ViewIt pins 0.36.0. | docs.rs lists `quick-xml ^0.41`, `zip ^8.6`, `serde`, `encoding_rs`, `byteorder`, numeric parsers, optional `chrono`; 99.71% documented. | MIT | crates.io/docs.rs latest 0.36.0 published 2026-07-06. | Not for DOCX. Already maps well to `Document::Xlsx`; reinforces keeping XLSX separate from DOCX choices. |
| SheetJS `xlsx` | Broad spreadsheet formats including XLSX | Parses spreadsheet bytes into workbook objects and writes spreadsheets; utility functions render to JSON/HTML tables. Not DOCX/PPTX. | Strong. README documents standalone browser scripts, ESM, Deno, Electron, Chrome extensions, React Native, web file input/drag-drop. | JS/WebView only, unless adding a JS bridge. | npm page says 7 dependencies, 6,563 dependents, 0.18.5 last published 4 years ago. | npm page does not clearly state text license in fetched content; it links license badge. Verify from package source before adoption. | Very mature/popular: 108 versions, broad demos and ecosystem support, but npm latest is 4 years old. | Not for DOCX. Could replace/augment XLSX preview in WebView, but would duplicate `calamine`. |

## Detailed Notes

### Rust `ooxmlsdk`

The strongest Rust-first alternative is `ooxmlsdk`. Its README describes it as a pure-Rust library for reading, editing, writing, and round-tripping Office Open XML documents such as `.docx`, `.xlsx`, and `.pptx`, with generated schema types and strongly typed package parts modeled after the .NET Open XML SDK. The usage example opens a `WordprocessingDocument` from a file and saves it; README says to use `SpreadsheetDocument` and `PresentationDocument` for `.xlsx` and `.pptx`.

This is not a renderer. It gives typed package and schema access. That is a good match if ViewIt wants better DOCX extraction, relationships, styles, headers/footers, images, and future PPTX/XLSX consistency. It is not a drop-in replacement for the current `DocxBlock` extraction because we would still need to traverse WordprocessingML and map runs, paragraphs, tables, numbering, and drawing/image relationships.

Risks are toolchain and API stability. The README states MSRV 1.88 for core crates and 1.92 for `ooxmlsdk-pdf`, Rust 2024 workspace, and pre-1.0 generated APIs that can still change. docs.rs lists dependencies including `quick-xml ^0.41`, optional `zip ^8.6`, `ooxmlsdk-derive`, `thiserror`, and generated modules under `parts` for Word, Spreadsheet, and Presentation package parts.

Sources: [GitHub README](https://github.com/KaiserY/ooxmlsdk), [docs.rs crate page](https://docs.rs/ooxmlsdk/latest/ooxmlsdk/), [docs.rs parts module](https://docs.rs/ooxmlsdk/latest/ooxmlsdk/parts/index.html), [crates.io API](https://crates.io/api/v1/crates/ooxmlsdk).

### Current `docx-rust`

`docx-rust` remains the most direct fit for the existing model. Its docs say it is a Rust library for parsing and generating DOCX files. The documented flow is `DocxFile::from_file` or `DocxFile::from_reader`, then `DocxFile::parse` to produce a `Docx` struct. Its `BodyContent` enum includes `Paragraph` and `Table`, and `Paragraph` exposes `text()`, `property`, and run contents. That maps cleanly to ViewIt's `Paragraph`, `ListItem`, and `Table` extraction.

Its limitations are visible from the surface: it is DOCX-only, not a renderer, and has low docs coverage. ViewIt already has to add heading/list heuristics and image placeholders, and it falls back when parse fails. Still, for Rust/Android and the current `Document::Docx` abstraction, it is the best low-disruption default.

Sources: [docs.rs crate page](https://docs.rs/docx-rust/latest/docx_rust/), [DocxFile docs](https://docs.rs/docx-rust/latest/docx_rust/struct.DocxFile.html), [BodyContent docs](https://docs.rs/docx-rust/latest/docx_rust/document/enum.BodyContent.html), [Paragraph docs](https://docs.rs/docx-rust/latest/docx_rust/document/struct.Paragraph.html), [crates.io API](https://crates.io/api/v1/crates/docx-rust).

### Current `zip` + `quick-xml` Fallback

The fallback is not a library alternative so much as an architectural choice: keep parsing only the OOXML parts ViewIt needs. In current code it opens the DOCX as ZIP, reads `word/document.xml`, and uses `quick-xml` events for `w:p`, `w:t`, `w:tbl`, `w:tr`, `w:tc`, `w:pStyle`, and `w:numPr`. That makes it small, controllable, and easy to map to `DocxBlock`.

The tradeoff is completeness. It ignores relationships, styles beyond direct style IDs, numbering level resolution, headers/footers, footnotes/endnotes, comments, images, tracked changes, alternate content, and package validation. It is still valuable as a rescue path and as the lowest-risk Android path.

Sources: ViewIt source `crates/fmt-office/src/lib.rs`, [docs.rs quick-xml 0.36.2](https://docs.rs/quick-xml/0.36.2/quick_xml/), [docs.rs zip 8.6.0](https://docs.rs/zip/8.6.0/zip/).

### JSZip

JSZip is a package primitive. npm and GitHub describe it as a JavaScript library for creating, reading, and editing `.zip` files. Since DOCX/XLSX/PPTX are ZIP packages, it is useful inside browser/WebView OOXML tools, but it does not understand WordprocessingML, SpreadsheetML, PresentationML, relationships, styles, or rendering.

Use it only if ViewIt moves some OOXML parsing/rendering to the WebView or adopts packages that depend on it. It does not replace `docx-rust` or `ooxmlsdk` for Rust parsing.

Sources: [npm package](https://www.npmjs.com/package/jszip), [GitHub README](https://github.com/Stuk/jszip).

### `@ooxml-tools/file`

`@ooxml-tools/file` is an OOXML package helper. Its README says it reads/writes Office Open XML files in nodejs/browser, supports `.docx` and `.xlsx`, and marks `.pptx` as coming soon. The usage accepts a JSZip instance, loads an OOXML path, then opens a format-specific wrapper and lists package files. CLI commands include `init`, `pack`, `unpack`, `list`, `read`, and `write`.

That makes it stronger than bare JSZip for package conventions, but it still is not a DOCX semantic parser or renderer. It could help a WebView implementation inspect or edit package parts, but ViewIt would still need a mapper to `Document::Docx`.

Sources: [GitHub README](https://github.com/ooxml-tools/file), [npm package](https://www.npmjs.com/package/@ooxml-tools/file).

### `docx-preview` / docxjs

`docx-preview` is the best browser-first rendering candidate. Its README states the goal is rendering/converting DOCX into HTML while preserving HTML semantics as much as possible, limited by HTML capabilities. Its API accepts `Blob`, `ArrayBuffer`, or `Uint8Array`, can render headers, footers, footnotes, endnotes, comments, altChunks, fonts, page breaks, and images, and returns a `WordDocument` from `renderAsync`. The README explicitly says only `renderAsync` is stable; `parseAsync` and `renderDocument` are experimental/internal and parsing/rendering internals may change.

For ViewIt, this is a product choice: if DOCX should look like a document in a WebView, `docx-preview` is more capable than `Document::Docx`. If we need the current structured Rust model, it is a poor fit because the stable output is DOM/HTML, not `DocxBlock`.

Sources: [GitHub README](https://github.com/VolodymyrBaydalka/docxjs), [npm package](https://www.npmjs.com/package/docx-preview).

### `mammoth`

Mammoth is a semantic DOCX-to-HTML converter. Its README says it converts DOCX to simple clean HTML using semantic information and intentionally ignores styling details instead of exactly copying layout. Supported features include headings, lists, style maps, tables, footnotes/endnotes, images, bold/italic/underline/strike/superscript/subscript, links, line breaks, text boxes, and comments. It also supports `extractRawText`.

For ViewIt, Mammoth's HTML output could be displayed directly in WebView or post-processed into `DocxBlock`, but post-processing HTML back into blocks is lossy and adds a sanitization requirement. Its README warns it performs no sanitization and that crafted documents may cause high CPU or memory usage.

Sources: [GitHub README](https://github.com/mwilliamson/mammoth.js), [npm package](https://www.npmjs.com/package/mammoth).

## XLSX Implications

ViewIt's current XLSX path uses `calamine`, which is still the right Rust-side choice for structured spreadsheet previews. `calamine` docs describe it as a pure Rust Excel/OpenDocument reader that reads cell values and VBA projects. It supports `Xlsx` for XLSX/XLSM/XLAM, `Xls`, `Xlsb`, and `Ods`, and exposes worksheet ranges, formulas, defined names, optional picture support, and sheet metadata.

If ViewIt moves XLSX into WebView JS, SheetJS is the obvious primary candidate: it parses spreadsheet bytes in browsers, Node, Deno, Electron, React Native, and many other environments, and can produce JSON or HTML tables. But it would duplicate `calamine` and is unrelated to DOCX.

Sources: [docs.rs calamine](https://docs.rs/calamine/latest/calamine/), [crates.io calamine API](https://crates.io/api/v1/crates/calamine), [npm SheetJS xlsx](https://www.npmjs.com/package/xlsx).

## PPTX Implications

The current custom PPTX path is analogous to the DOCX fallback: ZIP + XML extraction for slide count/title/body. `ooxmlsdk` is the only researched Rust option here that claims first-class `.pptx` package/schema support. `@ooxml-tools/file` explicitly marks PPTX as coming soon, so it is not ready as a PPTX package helper. `docx-preview` and Mammoth are DOCX-only; JSZip can only unpack the package.

If PPTX fidelity becomes important, likely paths are either a WebView renderer dependency dedicated to PPTX or an `ooxmlsdk`-based typed extractor. For ViewIt's current `Document::Pptx { slide_count, slides, asset_path, stream_url }`, the custom parser remains adequate unless we need relationships/images/layout.

Sources: [ooxmlsdk README](https://github.com/KaiserY/ooxmlsdk), [ooxml-tools/file README](https://github.com/ooxml-tools/file), ViewIt source `crates/fmt-office/src/pptx.rs`.

## Recommendation

Keep the current Rust model for near-term DOCX. Upgrade within that model only if a specific bug class requires it:

1. For better parse coverage while keeping `Document::Docx`, prototype `ooxmlsdk` behind a feature flag once ViewIt's Rust toolchain can satisfy its MSRV.
2. For better visual fidelity, add a separate WebView DOCX renderer path using `docx-preview`, not as a replacement for the Rust parser.
3. For semantic HTML export/display, evaluate `mammoth`, but sanitize output and isolate processing for untrusted documents.
4. Keep `calamine` for XLSX unless a WebView spreadsheet experience becomes a product requirement.
5. Treat JSZip and `@ooxml-tools/file` as building blocks only, not parser/rendering solutions.
