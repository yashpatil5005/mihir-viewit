//! ViewIt core: file sniffing + dispatch table.
//!
//! Per plan §2: "Rust does structural parsing/decoding; Svelte owns layout and rendering."
//! Per plan §3: feature-gated per ``fmt-*`` crate so mobile builds can keep small.
//!
//! Re-exports the shared `viewit-core-types` crate so downstream consumers
//! (the Tauri apps) only need one dependency instead of two.

pub use viewit_core_types::{Document, Error, Format, Suggestion};

/// Entry point invoked by Tauri commands and by WASM.
///
/// `bytes` is the file content; `ext` is the lowercase extension (no dot);
/// `name` is the user-facing filename for display. The dispatch is byte-sniff
/// first (for magic-numbered formats), extension fallback.
pub fn open(bytes: &[u8], ext: &str, name: &str) -> Result<Document, Error> {
    let format = sniff(bytes, ext);
    dispatch(format, bytes, ext, name)
}

/// Sniff a format from a little bit of magic-byte sniffing plus extension.
/// Phase 1 priority is PlainText — the rest return `Unsupported` and become
/// `Placeholder` documents so the runtime path can be wired up first.
pub fn sniff(bytes: &[u8], ext: &str) -> Format {
    if let Some(f) = sniff_magic(bytes) {
        return f;
    }
    sniff_ext(ext)
}

fn sniff_magic(bytes: &[u8]) -> Option<Format> {
    if bytes.len() < 4 {
        return None;
    }
    let p = &bytes[..bytes.len().min(16)];
    Some(match p {
        // PDF: %PDF-
        [0x25, 0x50, 0x44, 0x46, 0x2D, ..] => Format::Pdf,
        // PNG
        [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, ..] => Format::ImagePng,
        // JPEG SOI
        [0xFF, 0xD8, 0xFF, ..] => Format::ImageJpg,
        // GIF87a / GIF89a
        [0x47, 0x49, 0x46, 0x38, 0x37, 0x61, ..] | [0x47, 0x49, 0x46, 0x38, 0x39, 0x61, ..] => Format::ImageGif,
        // BMP "BM"
        [0x42, 0x4D, ..] => Format::ImageBmp,
        // WebP: "RIFF....WEBP"
        [0x52, 0x49, 0x46, 0x46, _, _, _, _, 0x57, 0x45, 0x42, 0x50, ..] => Format::ImageWebp,
        // HEIC/HEIF: ISO BMFF ftyp box at offset 4
        [_, _, _, _, 0x66, 0x74, 0x79, 0x70, 0x68, 0x65, 0x69, 0x63, ..]
        | [_, _, _, _, 0x66, 0x74, 0x79, 0x70, 0x68, 0x65, 0x69, 0x78, ..]
        | [_, _, _, _, 0x66, 0x74, 0x79, 0x70, 0x68, 0x65, 0x76, 0x63, ..]
        | [_, _, _, _, 0x66, 0x74, 0x79, 0x70, 0x68, 0x65, 0x69, 0x6D, ..]
        | [_, _, _, _, 0x66, 0x74, 0x79, 0x70, 0x6D, 0x69, 0x66, 0x31, ..] => Format::ImageHeic,
        // PSD: "8BPS"
        [0x38, 0x42, 0x50, 0x53, ..] => Format::ImagePsd,
        // ZIP signature (also docx/xlsx/pptx/odt/ods/odp/epub/iWork — extension resolves those)
        [0x50, 0x4B, 0x03, 0x04, ..] | [0x50, 0x4B, 0x05, 0x06, ..] => Format::ArchiveZip,
        // gzip (so tar.gz or single-file gz)
        [0x1F, 0x8B, ..] => Format::ArchiveTarGz,
        // 7z
        [0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C, ..] => Format::Archive7z,
        // Rar
        [0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, ..] => Format::ArchiveRar,
        _ => return None,
    })
}

fn sniff_ext(ext: &str) -> Format {
    match ext {
        "txt" | "log" | "text" => Format::PlainText,
        "md" | "markdown" | "mdown" | "mkdn" => Format::Markdown,
        "json" => Format::Json,
        "csv" | "tsv" => Format::Csv,
        "pdf" => Format::Pdf,
        "png" => Format::ImagePng,
        "jpg" | "jpeg" => Format::ImageJpg,
        "webp" => Format::ImageWebp,
        "gif" => Format::ImageGif,
        "bmp" => Format::ImageBmp,
        "tif" | "tiff" => Format::ImageTiff,
        "svg" => Format::ImageSvg,
        "heic" | "heif" => Format::ImageHeic,
        "psd" => Format::ImagePsd,
        "epub" => Format::Epub,
        "zip" => Format::ArchiveZip,
        "tar" => Format::ArchiveTar,
        "tgz" | "gz" => Format::ArchiveTarGz,
        "7z" => Format::Archive7z,
        "rar" => Format::ArchiveRar,
        "docx" => Format::Docx,
        "xlsx" => Format::Xlsx,
        "pptx" => Format::Pptx,
        "odt" => Format::Odt,
        "ods" => Format::Ods,
        "odp" => Format::Odp,
        "rtf" => Format::Rtf,
        "pages" => Format::IworkPages,
        "numbers" => Format::IworkNumbers,
        "key" => Format::IworkKey,
        "plist" => Format::Plist,
        "ics" => Format::Ics,
        "vcf" => Format::Vcf,
        "desktop" => Format::Code,
        "rs" | "ts" | "js" | "py" | "go" | "c" | "cpp" | "h" | "hpp" | "java" | "kt"
        | "swift" | "sh" | "sql" | "lua" | "php" | "rb" | "ex" | "exs" | "erl" | "hs"
        | "ml" | "clj" | "cljs" | "scala" | "r" | "jl" | "vim" | "ps1" | "bat" => Format::Code,
        _ => Format::Unsupported,
    }
}

/// Dispatch on the sniffed format. Each branch delegates to the active
/// feature's crate. If the feature for that format is disabled at compile
/// time, we return Placeholder instead — keeps the binary lean.
pub fn dispatch(format: Format, bytes: &[u8], ext: &str, name: &str) -> Result<Document, Error> {
    match format {
        Format::PlainText | Format::Code => {
            return parse_text_like(bytes, format, name);
        }
        Format::Markdown | Format::Json | Format::Csv
        | Format::Plist | Format::Ics | Format::Vcf => {
            return parse_text_like(bytes, format, name);
        }
        Format::Pdf => {
            #[cfg(feature = "fmt-pdf")]
            {
                return viewit_fmt_pdf::parse(bytes, format, name).map_err(Error::from_parse);
            }
            #[cfg(not(feature = "fmt-pdf"))]
            return Ok(Document::Placeholder { format, name: name.to_string(), byte_len: bytes.len() });
        }
        Format::ImagePng | Format::ImageJpg | Format::ImageWebp | Format::ImageGif
        | Format::ImageBmp | Format::ImageTiff | Format::ImageSvg | Format::ImageHeic
        | Format::ImagePsd => {
            // Per plan §5: native webview decoder. Rust emits the `Image`
            // variant; the frontend uses `<img src=...>` directly.
            // (Phase 2.1 — zero Rust deps added.)
            return Ok(Document::Image {
                format,
                byte_len: bytes.len(),
                name: name.to_string(),
            });
        }
        Format::Epub => {
            #[cfg(feature = "fmt-ebook")]
            {
                return viewit_fmt_ebook::parse(bytes, format, name).map_err(Error::from_parse);
            }
            #[cfg(not(feature = "fmt-ebook"))]
            return Ok(Document::Placeholder { format, name: name.to_string(), byte_len: bytes.len() });
        }
        Format::ArchiveZip | Format::ArchiveTar | Format::ArchiveTarGz | Format::Archive7z => {
            #[cfg(feature = "fmt-archive")]
            {
                return viewit_fmt_archive::parse(bytes, format, name).map_err(Error::from_parse);
            }
            #[cfg(not(feature = "fmt-archive"))]
            return Ok(Document::Placeholder { format, name: name.to_string(), byte_len: bytes.len() });
        }
        Format::ArchiveRar => {
            // Per ADR 0004: RAR5 omitted entirely from v1 with friendly hand-off.
            return Ok(Document::Unsupported {
                format,
                reason: "RAR5 isn't supported inside ViewIt yet — proprietary compression format. Open with your installed extractor.".into(),
                suggestion: Suggestion::OpenWithExternal,
            });
        }
        Format::Docx | Format::Xlsx | Format::Pptx | Format::Odt | Format::Ods | Format::Odp => {
            #[cfg(feature = "fmt-office")]
            {
                return viewit_fmt_office::parse(bytes, format, name).map_err(Error::from_parse);
            }
            #[cfg(not(feature = "fmt-office"))]
            return Ok(Document::Placeholder { format, name: name.to_string(), byte_len: bytes.len() });
        }
        Format::Rtf => {
            #[cfg(feature = "fmt-text")]
            {
                return viewit_fmt_text::parse_text(bytes, format, name).map_err(Error::from_parse);
            }
            #[cfg(not(feature = "fmt-text"))]
            return Ok(Document::Placeholder { format, name: name.to_string(), byte_len: bytes.len() });
        }
        Format::IworkPages | Format::IworkNumbers | Format::IworkKey => {
            #[cfg(feature = "fmt-iwork")]
            {
                return viewit_fmt_iwork::parse(bytes, format, name).map_err(Error::from_parse);
            }
            #[cfg(not(feature = "fmt-iwork"))]
            return Ok(Document::Placeholder { format, name: name.to_string(), byte_len: bytes.len() });
        }
        // `Format` is `#[non_exhaustive]`, so a future variant MUST be handled
        // explicitly. Falling back to Unsupported here is the safety net.
        Format::Unsupported => return Ok(Document::Unsupported {
            format,
            reason: format!("No handler for file extension '.{}'", ext),
            suggestion: Suggestion::None,
        }),
        _ => return Ok(Document::Unsupported {
            format,
            reason: format!("Unknown format in this build: {:?}", format),
            suggestion: Suggestion::None,
        }),
    }
}

/// Helper: dispatch text-like formats (txt / md / json / csv / code). If the
/// `fmt-text` feature is enabled, delegate to `viewit_fmt_text::parse_text`.
/// Otherwise return a `Placeholder` so the runtime loop is still exercisable.
fn parse_text_like(bytes: &[u8], format: Format, name: &str) -> Result<Document, Error> {
    #[cfg(feature = "fmt-text")]
    {
        return viewit_fmt_text::parse_text(bytes, format, name).map_err(Error::from_parse);
    }
    #[cfg(not(feature = "fmt-text"))]
    {
        Ok(Document::Placeholder {
            format,
            name: name.to_string(),
            byte_len: bytes.len(),
        })
    }
}
