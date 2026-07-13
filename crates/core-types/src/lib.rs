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
    Epub,
    ArchiveZip,
    ArchiveTarGz,
    ArchiveTar,
    Archive7z,
    ArchiveRar,
    Docx,
    Xlsx,
    Pptx,
    Odt,
    Ods,
    Odp,
    Rtf,
    IworkPages,
    IworkNumbers,
    IworkKey,
    Unsupported,
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
    },
    /// Phase 2.1 — native webview pass-through. Rust emits the format + URI;
    /// the Svelte `ImageViewer.svelte` hands the URI to `<img>` /
    /// `<object>` (or `convertFileSrc` on Tauri hosts).
    Image {
        format: Format,
        #[serde(rename = "byte_len")]
        byte_len: usize,
        name: String,
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
    /// Phase 2.2 — PDF. Per ADR 0002: bundled pdfium arm64-v8a.
    /// Pages are rasterized to PNG bitmaps, base64-encoded as data URLs.
    /// First MAX_EAGER_PAGES are shipped eagerly; the rest via `pdf_page`.
    Pdf {
        page_count: usize,
        /// Base64 data URLs of the first N pages (PNG).
        pages: Vec<String>,
        #[serde(rename = "byte_len")]
        byte_len: usize,
    },
    Unsupported {
        format: Format,
        reason: String,
        suggestion: Suggestion,
    },
    /// Phase 1 only — placeholder while the wiring is exercised. Other
    /// variants are added in Phase 2+.
    Placeholder {
        format: Format,
        name: String,
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
