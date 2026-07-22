//! Shared ViewIt types — single source of truth: `Document`, `Format`, `Error`.
//!
//! Splits the design cycle: every `fmt-*` crate depends on this — never on
//! `viewit-core` itself, which would create a cycle.

use serde::{Deserialize, Serialize};
use thiserror::Error as ThiserrorError;

/// Format identifier, sniffed from magic bytes + extension. Each variant routes
/// to a specific crate's entry point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Format {
    PlainText,
    Markdown,
    Json,
    Csv,
    Code,
    Pdf,
    ImagePng,
    ImageJpg,
    ImageWebp,
    ImageGif,
    ImageBmp,
    ImageTiff,
    ImageSvg,
    ImageHeic,
    ImagePsd,
    ImageRaw,
    Epub,
    Mobi,
    Azw3,
    FictionBook,
    PalmDoc,
    Font,
    ArchiveZip,
    ArchiveTarGz,
    ArchiveTar,
    Archive7z,
    ArchiveRar,
    Docx,
    Xlsx,
    Xls,
    Pptx,
    Odt,
    Ods,
    Odp,
    Doc,
    Ppt,
    Rtf,
    IworkPages,
    IworkNumbers,
    IworkKey,
    Plist,
    Ics,
    Vcf,
    Video,
    Audio,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MediaKind {
    Video,
    Audio,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StreamKind {
    Pdf,
    Video,
    Audio,
}

/// Parsed document model. Svelte renders based on the variant.
///
/// Phase 1 has only `Text`; later phases add `Markdown`, `Pdf`, etc.
/// `Unsupported` lets the frontend render a graceful "Open with…" UI (ADR 0004).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Document {
    Text {
        content: String,
        encoding: String,
        #[serde(rename = "byte_len")]
        byte_len: usize,
        /// Phase 2.3 — true when content was truncated for transport safety.
        /// The frontend must request additional pages via `text_page` Tauri
        /// command (offset+len) when the user scrolls past the eager payload.
        truncated: bool,
        /// Streaming URL for large text files. Frontend fetches via HTTP instead of IPC.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stream_url: Option<String>,
    },
    /// Phase 2.1 — native webview pass-through. Rust emits the format + URI;
    /// the Svelte `ImageViewer.svelte` hands the URI to `<img>` /
    /// `<object>` (or `convertFileSrc` on Tauri hosts).
    Image {
        format: Format,
        #[serde(rename = "byte_len")]
        byte_len: usize,
        name: String,
        /// Materialized cache `file://` for Android WebView (`convertFileSrc`).
        #[serde(default)]
        asset_path: String,
        /// Streaming URL for direct WebView access. Preferred over asset_path.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stream_url: Option<String>,
    },
    /// Phase 2.4 — Markdown body (already converted to safe-ish HTML by
    /// `pulldown-cmark` on the Rust side).
    Markdown {
        html: String,
        #[serde(rename = "byte_len")]
        byte_len: usize,
    },
    /// Phase 2.5 — pretty-printed JSON (serde_json already re-serialized
    /// with `pretty`).
    Json {
        pretty: String,
        #[serde(rename = "byte_len")]
        byte_len: usize,
    },
    /// Phase 2.6 — CSV streaming. First N rows are sent eagerly; later rows
    /// are pulled via a separate command (`csv_page`) when the user scrolls
    /// near the bottom of the virtualized table.
    Csv {
        header: Vec<String>,
        preview_rows: Vec<Vec<String>>,
        total_rows_hint: Option<usize>,
        #[serde(rename = "byte_len")]
        byte_len: usize,
    },
    /// Phase 2.8 — EPUB. Per plan §4 "near-zero cost: zip + XHTML spine".
    /// Frontend renders via <iframe srcdoc> sandbox.
    Epub {
        title: String,
        author: Option<String>,
        /// First chapter's XHTML eagerly. Subsequent chapters via follow-up
        /// `epub_chapter` command in Phase 5 nav UI.
        first_chapter_xhtml: String,
        spine_len: usize,
        #[serde(rename = "byte_len")]
        byte_len: usize,
    },
    /// Phase 2.9 — archive listing. Drill-down via re-dispatch through
    /// `core::open` on the extracted entry.
    Archive {
        entries: Vec<ArchiveEntry>,
        format: Format,
        #[serde(rename = "byte_len")]
        byte_len: usize,
    },
    /// Phase 2.2 — PDF. ADR 0010: `native` + empty `pages` → WebView iframe + asset URL.
    /// ADR 0002 optional: raster `pages` via pdfium when `fmt-pdf` enabled.
    Pdf {
        page_count: usize,
        pages: Vec<String>,
        #[serde(rename = "byte_len")]
        byte_len: usize,
        #[serde(default)]
        native: bool,
        name: String,
        /// Streaming URL for direct WebView/pdf.js access.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stream_url: Option<String>,
    },
    /// ADR 0010 — video/audio; `asset_path` is host cache file for `convertFileSrc`.
    Media {
        format: Format,
        media_kind: MediaKind,
        name: String,
        #[serde(rename = "byte_len")]
        byte_len: usize,
        #[serde(default)]
        asset_path: String,
        #[serde(default)]
        ext: String,
        /// Streaming URL for direct WebView access. Preferred over asset_path.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stream_url: Option<String>,
    },
    /// PDF / media materialized to app cache (Android custom protocol workaround).
    /// Deprecated: use `Pdf`/`Media` with `stream_url` instead.
    StreamFile {
        asset_path: String,
        mime: String,
        name: String,
        #[serde(rename = "byte_len")]
        byte_len: usize,
        stream_kind: StreamKind,
        /// Streaming URL for direct WebView access.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stream_url: Option<String>,
    },
    Unsupported {
        format: Format,
        reason: String,
        suggestion: Suggestion,
    },
    /// Phase 3.3 — PPTX slide listing with title + body text per slide.
    Pptx {
        slide_count: usize,
        slides: Vec<PptxSlide>,
        #[serde(rename = "byte_len")]
        byte_len: usize,
        /// Materialized `.pptx` on disk for client-side `pptx-viewer` (Android SAF).
        #[serde(default)]
        asset_path: String,
        /// Streaming URL for direct client-side viewer access.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stream_url: Option<String>,
    },
    /// Phase 3.1 — DOCX structured: paragraphs (with optional heading level),
    /// tables (rows × cells), and lazily-rendered text runs.
    Docx {
        blocks: Vec<DocxBlock>,
        #[serde(rename = "byte_len")]
        byte_len: usize,
    },
    /// Phase 3.2 — XLSX structured sheet listing. Each sheet has its own
    /// header + preview rows; the frontend renders a tab strip + virtualized grid.
    Xlsx {
        sheets: Vec<XlsxSheet>,
        #[serde(rename = "byte_len")]
        byte_len: usize,
    },
    /// Phase 1 only — placeholder while the wiring is exercised. Other
    /// variants are added in Phase 2+.
    Placeholder {
        format: Format,
        name: String,
        #[serde(rename = "byte_len")]
        byte_len: usize,
    },
    /// Font metadata — TTF/OTF/WOFF family, weight, style info.
    Font {
        family_name: String,
        weight: u16,
        is_italic: bool,
        format: Format,
        #[serde(rename = "byte_len")]
        byte_len: usize,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Suggestion {
    OpenWithExternal,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveEntry {
    pub name: String,
    pub size: u64,
    pub is_dir: bool,
    pub compressed_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum DocxBlock {
    Paragraph { text: String, heading: Option<u8> },
    ListItem { text: String, level: u8 },
    Table { rows: Vec<Vec<String>> },
    Image { name: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PptxSlide {
    pub title: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XlsxSheet {
    pub name: String,
    pub header: Vec<String>,
    pub preview_rows: Vec<Vec<String>>,
    pub total_rows_hint: Option<usize>,
}

/// Errors that any fmt-* crate may return. Uniform so the frontend can rely
/// on one shape.
#[derive(Debug, ThiserrorError)]
pub enum Error {
    #[error("I/O error reading file: {0}")]
    Io(#[from] std::io::Error),
    #[error("unsupported format: {0:?}")]
    UnsupportedFormat(Format),
    #[error("format-specific parse error: {0}")]
    Parse(String),
}

impl Error {
    /// Adapter so fmt-* crates can keep their own error types without us
    /// depending on them tightly here.
    pub fn from_parse<E: std::fmt::Display>(e: E) -> Self {
        Error::Parse(e.to_string())
    }
}
