//! ViewIt core: file sniffing + dispatch table.
//!
//! Per plan §2: "Rust does structural parsing/decoding; Svelte owns layout and rendering."
//! Per plan §3: feature-gated per ``fmt-*`` crate so mobile builds can keep small.
//!
//! Re-exports the shared `viewit-core-types` crate so downstream consumers
//! (the Tauri apps) only need one dependency instead of two.

pub use viewit_core_types::{Document, Error, Format, Suggestion};

pub mod io;
pub use io::reject_if_too_large;

/// Entry point invoked by Tauri commands and by WASM.
///
/// `bytes` is the file content; `ext` is the lowercase extension (no dot);
/// `name` is the user-facing filename for display. The dispatch is byte-sniff
/// first (for magic-numbered formats), extension fallback.
/// Max bytes read into memory for a single open (picker / share). Avoids OOM on video etc.
pub const OPEN_BYTES_CAP: usize = 32 * 1024 * 1024;

pub fn open(bytes: &[u8], ext: &str, name: &str) -> Result<Document, Error> {
    if bytes.len() > OPEN_BYTES_CAP {
        return Ok(Document::Unsupported {
            format: Format::Unsupported,
            reason: format!(
                "File is {:.1} MB — ViewIt loads up to {} MB in memory. Open with another app.",
                bytes.len() as f64 / 1_048_576.0,
                OPEN_BYTES_CAP / 1_048_576
            ),
            suggestion: Suggestion::OpenWithExternal,
        });
    }
    let format = sniff(bytes, ext);
    dispatch(format, bytes, ext, name)
}

/// Sniff a format from a little bit of magic-byte sniffing plus extension.
/// Phase 1 priority is PlainText — the rest return `Unsupported` and become
/// `Placeholder` documents so the runtime path can be wired up first.
pub fn sniff(bytes: &[u8], ext: &str) -> Format {
    if let Some(f) = sniff_magic(bytes, ext) {
        return f;
    }
    sniff_ext(ext)
}

/// Extension hint from magic only (content:// shares often lack a filename).
pub fn ext_hint_from_magic(bytes: &[u8]) -> Option<&'static str> {
    ext_from_sniff(bytes, "")
}

pub fn ext_from_sniff(bytes: &[u8], ext_hint: &str) -> Option<&'static str> {
    if bytes.is_empty() {
        return None;
    }
    let cap = bytes.len().min(256 * 1024);
    let f = sniff(&bytes[..cap], ext_hint);
    Some(match f {
        Format::Pdf => "pdf",
        Format::PlainText | Format::Code => "txt",
        Format::Markdown => "md",
        Format::Json => "json",
        Format::Csv => "csv",
        Format::Docx => "docx",
        Format::Xlsx => "xlsx",
        Format::Pptx => "pptx",
        Format::Doc => "doc",
        Format::Xls => "xls",
        Format::Ppt => "ppt",
        Format::ImagePng => "png",
        Format::ImageJpg => "jpg",
        Format::ImageGif => "gif",
        Format::ImageWebp => "webp",
        Format::ImageRaw => "dng",
        Format::ImageHeic => "heic",
        Format::ImageBmp => "bmp",
        Format::ImageTiff => "tiff",
        Format::ImagePsd => "psd",
        Format::Video => "mp4",
        Format::Audio => "mp3",
        Format::ArchiveZip => "zip",
        Format::ArchiveTarGz => "gz",
        Format::ArchiveTar => "tar",
        Format::ArchiveTarBz2 => "bz2",
        Format::ArchiveTarXz => "xz",
        Format::ArchiveTarZstd => "zst",
        Format::ArchiveTarLz4 => "lz4",
        Format::ArchiveTarLzma => "lzma",
        Format::Archive7z => "7z",
        Format::ArchiveRar => "rar",
        Format::IworkPages => "pages",
        Format::IworkNumbers => "numbers",
        Format::IworkKey => "key",
        _ => return None,
    })
}

/// ZIP magic matches docx/xlsx/pptx/odt/ods/odp/epub/iWork — extension wins over ArchiveZip.
fn sniff_zip_as_office_or_epub(bytes: &[u8], ext: &str) -> Format {
    match ext {
        "docx" => Format::Docx,
        "xlsx" => Format::Xlsx,
        "pptx" => Format::Pptx,
        "odt" => Format::Odt,
        "ods" => Format::Ods,
        "odp" => Format::Odp,
        "epub" => Format::Epub,
        "pages" => Format::IworkPages,
        "numbers" => Format::IworkNumbers,
        "key" => Format::IworkKey,
        _ => sniff_zip_inner(bytes).unwrap_or(Format::ArchiveZip),
    }
}

/// OOXML / ODF / EPUB / iWork inside a ZIP (when extension missing or wrong).
fn sniff_zip_inner(bytes: &[u8]) -> Option<Format> {
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).ok()?;
    let mut has_content_types = false;
    let mut has_word = false;
    let mut has_xl = false;
    let mut has_ppt = false;
    let mut has_epub = false;
    for i in 0..archive.len() {
        let Ok(file) = archive.by_index(i) else {
            continue;
        };
        let name = file.name();
        if name == "[Content_Types].xml" {
            has_content_types = true;
        }
        if name.starts_with("word/") {
            has_word = true;
        }
        if name.starts_with("xl/") {
            has_xl = true;
        }
        if name.starts_with("ppt/") {
            has_ppt = true;
        }
        if name == "META-INF/container.xml" {
            has_epub = true;
        }
    }
    if has_epub {
        return Some(Format::Epub);
    }
    if has_content_types {
        if has_xl {
            return Some(Format::Xlsx);
        }
        if has_word {
            return Some(Format::Docx);
        }
        if has_ppt {
            return Some(Format::Pptx);
        }
    }
    None
}

fn sniff_magic(bytes: &[u8], ext: &str) -> Option<Format> {
    if bytes.len() < 4 {
        return None;
    }
    // ISO BMFF (MP4/MOV/M4V/3GP/F4V/HEVC/MJPEG)
    if bytes.len() >= 12 && bytes[4..8] == *b"ftyp" {
        // Check for AVIF/AVIS brands first (image, not video)
        if bytes.len() >= 12 {
            let brand = &bytes[8..12];
            if brand == b"avif" || brand == b"avis" {
                return Some(Format::ImageHeic);
            }
        }
        return Some(Format::Video);
    }
    // RIFF container — AVI or WAVE audio
    if bytes.len() >= 12 && bytes[..4] == *b"RIFF" {
        if bytes[8..12] == *b"AVI " {
            return Some(Format::Video);
        }
        if bytes[8..12] == *b"WAVE" {
            return Some(Format::Audio);
        }
    }
    // EBML header — Matroska (MKV/WebM)
    if bytes.len() >= 4 && bytes[..4] == [0x1A, 0x45, 0xDF, 0xA3] {
        return Some(Format::Video);
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
        [0x47, 0x49, 0x46, 0x38, 0x37, 0x61, ..] | [0x47, 0x49, 0x46, 0x38, 0x39, 0x61, ..] => {
            Format::ImageGif
        }
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
        // TIFF LE / BE (but check for RAW extensions first)
        [0x49, 0x49, 0x2A, 0x00, ..] | [0x4D, 0x4D, 0x00, 0x2A, ..] => {
            if matches!(
                ext,
                "nef" | "cr2" | "cr3" | "arw" | "orf" | "rw2" | "raf" | "srw" | "pef" | "dng"
            ) {
                Format::ImageRaw
            } else {
                Format::ImageTiff
            }
        }
        // ICO
        [0x00, 0x00, 0x01, 0x00, ..] => Format::ImagePng,
        // FLV: "FLV\x01"
        [0x46, 0x4C, 0x56, 0x01, ..] => Format::Video,
        // OGG container (OggS)
        [0x4F, 0x67, 0x67, 0x53, ..] => Format::Audio,
        // FLAC
        [0x66, 0x4C, 0x61, 0x43, ..] => Format::Audio,
        // MP3 frame sync (0xFF 0xFB or 0xFF 0xF3 or ID3 tag)
        [0xFF, 0xFB, ..] | [0xFF, 0xF3, ..] | [0xFF, 0xF2, ..] => Format::Audio,
        // ADTS AAC frame sync (0xFF 0xF1 = MPEG-4 no CRC, 0xFF 0xF9 = MPEG-4 with CRC)
        [0xFF, 0xF1, ..] | [0xFF, 0xF9, ..] => Format::Audio,
        // ID3 tag (MP3)
        [0x49, 0x44, 0x33, ..] => Format::Audio,
        // ASF/WMV: 0x30 0x26 0xB2 0x75 0x8E 0x66 0xCF 0x11
        [0x30, 0x26, 0xB2, 0x75, 0x8E, 0x66, 0xCF, 0x11, ..] => Format::Video,
        // MXF: 0x06 0x0E 0x2B 0x34 0x02
        [0x06, 0x0E, 0x2B, 0x34, 0x02, ..] => Format::Video,
        // MPEG-TS: sync byte 0x47 repeated at 188-byte intervals
        [0x47, _, _, _, _, _, _, _, _, _, _, _, _, _, _, 0x47, ..] => Format::Video,
        // MPEG-PS: starts with 0x00 0x00 0x01
        [0x00, 0x00, 0x01, ..] => Format::Video,
        // SWF: "FWS" or "CWS"
        [0x46, 0x57, 0x53, ..] | [0x43, 0x57, 0x53, ..] => Format::Video,
        // ZIP — extension + inner paths disambiguate Office / EPUB from plain zip.
        [0x50, 0x4B, 0x03, 0x04, ..] | [0x50, 0x4B, 0x05, 0x06, ..] => {
            return Some(sniff_zip_as_office_or_epub(bytes, ext));
        }
        // gzip (so tar.gz or single-file gz)
        [0x1F, 0x8B, ..] => Format::ArchiveTarGz,
        // bzip2
        [0x42, 0x5A, 0x68, ..] => Format::ArchiveTarBz2,
        // xz/lzma
        [0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00, ..] => Format::ArchiveTarXz,
        // zstd
        [0x28, 0xB5, 0x2F, 0xFD, ..] => Format::ArchiveTarZstd,
        // lz4
        [0x04, 0x22, 0x4D, 0x18, ..] => Format::ArchiveTarLz4,
        // 7z
        [0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C, ..] => Format::Archive7z,
        // Rar
        [0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, ..] => Format::ArchiveRar,
        _ => return None,
    })
}

pub fn is_video_ext(ext: &str) -> bool {
    matches!(
        ext,
        "mp4"
            | "m4v"
            | "webm"
            | "mkv"
            | "mov"
            | "avi"
            | "mpg"
            | "mpeg"
            | "3gp"
            | "wmv"
            | "flv"
            | "ts"
            | "asf"
            | "f4v"
            | "hevc"
            | "m2ts"
            | "m2v"
            | "mjpeg"
            | "mts"
            | "mxf"
            | "ogv"
            | "rm"
            | "swf"
            | "vob"
            | "wtv"
    )
}

pub fn is_audio_ext(ext: &str) -> bool {
    matches!(
        ext,
        "mp3"
            | "m4a"
            | "aac"
            | "flac"
            | "ogg"
            | "wav"
            | "wma"
            | "opus"
            | "8svx"
            | "ac3"
            | "aif"
            | "aiff"
            | "amb"
            | "au"
            | "avr"
            | "caf"
            | "cdda"
            | "cvs"
            | "cvsd"
            | "cvu"
            | "dts"
            | "dvms"
            | "fap"
            | "fssd"
            | "gsrt"
            | "hcom"
            | "htk"
            | "ima"
            | "ircam"
            | "m4r"
            | "maud"
            | "mp2"
            | "nist"
            | "oga"
            | "paf"
            | "prc"
            | "pvf"
            | "ra"
            | "sd2"
            | "sln"
            | "smp"
            | "snd"
            | "sndr"
            | "sndt"
            | "sou"
            | "sph"
            | "spx"
            | "tta"
            | "txw"
            | "vms"
            | "voc"
            | "vox"
            | "w64"
            | "wv"
            | "wve"
    )
}

pub fn is_stream_ext(ext: &str) -> bool {
    ext == "pdf" || is_video_ext(ext) || is_audio_ext(ext)
}

pub fn open_stream(ext: &str, name: &str) -> Result<Document, Error> {
    if ext == "pdf" {
        return Ok(Document::Pdf {
            page_count: 0,
            pages: vec![],
            byte_len: 0,
            native: true,
            name: name.to_string(),
            asset_path: String::new(),
            stream_url: None,
        });
    }
    if is_video_ext(ext) {
        return Ok(Document::Media {
            format: Format::Video,
            media_kind: viewit_core_types::MediaKind::Video,
            name: name.to_string(),
            byte_len: 0,
            asset_path: String::new(),
            ext: ext.to_string(),
            stream_url: None,
        });
    }
    if is_audio_ext(ext) {
        return Ok(Document::Media {
            format: Format::Audio,
            media_kind: viewit_core_types::MediaKind::Audio,
            name: name.to_string(),
            byte_len: 0,
            asset_path: String::new(),
            ext: ext.to_string(),
            stream_url: None,
        });
    }
    Err(Error::Parse(format!("not a stream format: .{}", ext)))
}

fn sniff_ext(ext: &str) -> Format {
    match ext {
        "txt" | "log" | "text" | "readme" | "gitignore" | "gitattributes" | "editorconfig"
        | "dockerignore" | "npmignore" | "nfo" | "srt" | "vtt" | "sub" | "rst" | "adoc" | "asc"
        | "strings" => Format::PlainText,
        "md" | "markdown" | "mdown" | "mkdn" | "mdx" => Format::Markdown,
        "json" | "jsonc" | "json5" | "ipynb" | "jsonl" | "ndjson" | "lock" => Format::Json,
        "csv" | "tsv" => Format::Csv,
        "yaml" | "yml" => Format::Code,
        "xml" | "xsd" | "xsl" | "xslt" => Format::Code,
        "html" | "htm" | "xhtml" => Format::Code,
        "css" | "scss" | "sass" | "less" => Format::Code,
        "ini" | "cfg" | "conf" | "config" | "env" | "toml" => Format::Code,
        "pdf" => Format::Pdf,
        "png" => Format::ImagePng,
        "jpg" | "jpeg" => Format::ImageJpg,
        "webp" => Format::ImageWebp,
        "gif" => Format::ImageGif,
        "bmp" => Format::ImageBmp,
        "tif" | "tiff" => Format::ImageTiff,
        "svg" | "svgz" => Format::ImageSvg,
        "heic" | "heif" | "avif" => Format::ImageHeic,
        "psd" => Format::ImagePsd,
        "dng" | "cr2" | "cr3" | "nef" | "arw" | "orf" | "rw2" | "raf" | "srw" | "pef" => {
            Format::ImageRaw
        }
        "ico" | "icns" | "jfif" => Format::ImagePng,
        // Additional image formats — mapped to ImageRaw for webview best-effort
        "cur" | "dds" | "erf" | "exr" | "fts" | "hdr" | "jp2" | "jpe" | "jps" | "mng" | "nrw"
        | "pam" | "pbm" | "pcd" | "pcx" | "pes" | "pfm" | "pgm" | "picon" | "pict" | "pnm"
        | "ppm" | "ras" | "sfw" | "sgi" | "tga" | "wbmp" | "wpg" | "x3f" | "xbm" | "xcf"
        | "xpm" | "xwd" => Format::ImageRaw,
        "epub" => Format::Epub,
        "mobi" => Format::Mobi,
        "azw3" => Format::Azw3,
        "fb2" => Format::FictionBook,
        "lrf" | "pdb" | "snb" => Format::PalmDoc,
        // Fonts
        "ttf" | "otf" | "woff" | "woff2" | "pfb" | "cff" | "dfont" | "sfd" | "ps" | "ttc" => {
            Format::Font
        }
        "zip" => Format::ArchiveZip,
        "tar" => Format::ArchiveTar,
        "tgz" | "gz" => Format::ArchiveTarGz,
        "tbz2" | "bz2" => Format::ArchiveTarBz2,
        "txz" | "xz" => Format::ArchiveTarXz,
        "tzst" | "zst" => Format::ArchiveTarZstd,
        "tar.lz4" | "lz4" => Format::ArchiveTarLz4,
        "tlz" | "lzma" => Format::ArchiveTarLzma,
        "7z" => Format::Archive7z,
        "rar" => Format::ArchiveRar,
        "docx" | "docm" | "dotx" | "dotm" => Format::Docx,
        "xlsx" | "xlsm" => Format::Xlsx,
        "xlsb" => Format::Xlsb,
        "xls" => Format::Xls,
        "pptx" | "pptm" | "potx" => Format::Pptx,
        "odt" | "ott" => Format::Odt,
        "ods" => Format::Ods,
        "odp" => Format::Odp,
        "djvu" | "djv" => Format::ImageRaw, // DjVu — treat as image-like
        "doc" => Format::Doc,
        "ppt" => Format::Ppt,
        "rtf" => Format::Rtf,
        "pages" => Format::IworkPages,
        "numbers" => Format::IworkNumbers,
        "key" => Format::IworkKey,
        "plist" => Format::Plist,
        "ics" => Format::Ics,
        "vcf" => Format::Vcf,
        "desktop" => Format::Code,
        "mp4" | "m4v" | "webm" | "mkv" | "mov" | "avi" | "mpg" | "mpeg" | "3gp" | "wmv" | "flv"
        | "ts" | "asf" | "f4v" | "hevc" | "m2ts" | "m2v" | "mjpeg" | "mts" | "mxf" | "ogv"
        | "rm" | "swf" | "vob" | "wtv" => Format::Video,
        "mp3" | "m4a" | "aac" | "flac" | "ogg" | "wav" | "wma" | "opus" | "8svx" | "ac3"
        | "aif" | "aiff" | "amb" | "au" | "avr" | "caf" | "cdda" | "cvs" | "cvsd" | "cvu"
        | "dts" | "dvms" | "fap" | "fssd" | "gsrt" | "hcom" | "htk" | "ima" | "ircam" | "m4r"
        | "maud" | "mp2" | "nist" | "oga" | "paf" | "prc" | "pvf" | "ra" | "sd2" | "sln"
        | "smp" | "snd" | "sndr" | "sndt" | "sou" | "sph" | "spx" | "tta" | "txw" | "vms"
        | "voc" | "vox" | "w64" | "wv" | "wve" => Format::Audio,
        "rs" | "tsx" | "jsx" | "mjs" | "cjs" | "js" | "py" | "pyw" | "go" | "c" | "cc" | "cpp"
        | "cxx" | "h" | "hpp" | "hh" | "java" | "kt" | "kts" | "swift" | "sh" | "bash" | "zsh"
        | "fish" | "sql" | "lua" | "php" | "rb" | "ex" | "exs" | "erl" | "hrl" | "hs" | "ml"
        | "mli" | "clj" | "cljs" | "scala" | "sc" | "r" | "jl" | "vim" | "ps1" | "bat" | "cmd"
        | "cs" | "fs" | "fsx" | "dart" | "zig" | "nim" | "v" | "sv" | "vhd" | "vhdl" | "asm"
        | "s" | "gradle" | "cmake" | "make" | "mk" | "dockerfile" | "proto" | "graphql" | "gql"
        | "vue" | "svelte" | "astro" | "wasm" | "wat" | "patch" | "diff" | "class" | "htaccess"
        | "kml" | "kmz" => Format::Code,
        _ => Format::Unsupported,
    }
}

/// Dispatch on the sniffed format. Each branch delegates to the active
/// feature's crate. If the feature for that format is disabled at compile
/// time, we return Placeholder instead — keeps the binary lean.
pub fn dispatch(format: Format, bytes: &[u8], ext: &str, name: &str) -> Result<Document, Error> {
    match format {
        Format::PlainText | Format::Code => parse_text_like(bytes, format, name),
        Format::Markdown
        | Format::Json
        | Format::Csv
        | Format::Plist
        | Format::Ics
        | Format::Vcf => parse_text_like(bytes, format, name),
        Format::Pdf => {
            #[cfg(feature = "fmt-pdf")]
            {
                return viewit_fmt_pdf::parse(bytes, format, name).map_err(Error::from_parse);
            }
            #[cfg(not(feature = "fmt-pdf"))]
            return Ok(Document::Pdf {
                page_count: 0,
                pages: vec![],
                byte_len: bytes.len(),
                native: true,
                name: name.to_string(),
                asset_path: String::new(),
                stream_url: None,
            });
        }
        Format::Video | Format::Audio => {
            let kind = if format == Format::Video {
                viewit_core_types::MediaKind::Video
            } else {
                viewit_core_types::MediaKind::Audio
            };
            Ok(Document::Media {
                format,
                media_kind: kind,
                name: name.to_string(),
                byte_len: bytes.len(),
                asset_path: String::new(),
                ext: ext.to_string(),
                stream_url: None,
            })
        }
        Format::ImagePng
        | Format::ImageJpg
        | Format::ImageWebp
        | Format::ImageGif
        | Format::ImageBmp
        | Format::ImageTiff
        | Format::ImageSvg
        | Format::ImageHeic
        | Format::ImagePsd
        | Format::ImageRaw => {
            // Per plan §5: native webview decoder. Rust emits the `Image`
            // variant; the frontend uses `<img src=...>` directly.
            // (Phase 2.1 — zero Rust deps added.)
            Ok(Document::Image {
                format,
                byte_len: bytes.len(),
                name: name.to_string(),
                asset_path: String::new(),
                stream_url: None,
            })
        }
        Format::Epub | Format::Mobi | Format::Azw3 => {
            #[cfg(feature = "fmt-ebook")]
            {
                return viewit_fmt_ebook::parse(bytes, format, name).map_err(Error::from_parse);
            }
            #[cfg(not(feature = "fmt-ebook"))]
            return Ok(Document::Placeholder {
                format,
                name: name.to_string(),
                byte_len: bytes.len(),
            });
        }
        Format::FictionBook | Format::PalmDoc => {
            // Non-EPUB/MOBI ebooks — show placeholder with format info
            Ok(Document::Placeholder {
                format,
                name: name.to_string(),
                byte_len: bytes.len(),
            })
        }
        Format::Font => {
            #[cfg(feature = "fmt-font")]
            {
                return viewit_fmt_font::parse(bytes, format, name).map_err(Error::from_parse);
            }
            #[cfg(not(feature = "fmt-font"))]
            return Ok(Document::Placeholder {
                format,
                name: name.to_string(),
                byte_len: bytes.len(),
            });
        }
        Format::ArchiveZip
        | Format::ArchiveTar
        | Format::ArchiveTarGz
        | Format::Archive7z
        | Format::ArchiveTarBz2
        | Format::ArchiveTarXz
        | Format::ArchiveTarZstd
        | Format::ArchiveTarLz4
        | Format::ArchiveTarLzma
        | Format::ArchiveRar => {
            #[cfg(feature = "fmt-archive")]
            {
                return viewit_fmt_archive::parse(bytes, format, name).map_err(Error::from_parse);
            }
            #[cfg(not(feature = "fmt-archive"))]
            return Ok(Document::Placeholder {
                format,
                name: name.to_string(),
                byte_len: bytes.len(),
            });
        }
        Format::Docx
        | Format::Xlsx
        | Format::Xls
        | Format::Xlsb
        | Format::Pptx
        | Format::Odt
        | Format::Ods
        | Format::Odp
        | Format::Doc
        | Format::Ppt => {
            #[cfg(feature = "fmt-office")]
            {
                return viewit_fmt_office::parse(bytes, format, name).map_err(Error::from_parse);
            }
            #[cfg(not(feature = "fmt-office"))]
            return Ok(Document::Placeholder {
                format,
                name: name.to_string(),
                byte_len: bytes.len(),
            });
        }
        Format::Rtf => {
            #[cfg(feature = "fmt-text")]
            {
                viewit_fmt_text::parse_text(bytes, format, name).map_err(Error::from_parse)
            }
            #[cfg(not(feature = "fmt-text"))]
            return Ok(Document::Placeholder {
                format,
                name: name.to_string(),
                byte_len: bytes.len(),
            });
        }
        Format::IworkPages | Format::IworkNumbers | Format::IworkKey => {
            #[cfg(feature = "fmt-iwork")]
            {
                return viewit_fmt_iwork::parse(bytes, format, name).map_err(Error::from_parse);
            }
            #[cfg(not(feature = "fmt-iwork"))]
            return Ok(Document::Placeholder {
                format,
                name: name.to_string(),
                byte_len: bytes.len(),
            });
        }
        // `Format` is `#[non_exhaustive]`, so a future variant MUST be handled
        // explicitly. Falling back to Unsupported here is the safety net.
        Format::Unsupported => {
            let reason = if is_video_ext(ext) {
                "Video isn't viewable in ViewIt yet — use Open with… to play in another app.".into()
            } else {
                format!("No handler for file extension '.{}'", ext)
            };
            Ok(Document::Unsupported {
                format,
                reason,
                suggestion: if is_video_ext(ext) {
                    Suggestion::OpenWithExternal
                } else {
                    Suggestion::None
                },
            })
        }
        _ => Ok(Document::Unsupported {
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
        viewit_fmt_text::parse_text(bytes, format, name).map_err(Error::from_parse)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zip_magic_xlsx_ext_is_office_not_archive() {
        let pk = &[0x50, 0x4B, 0x03, 0x04, 0x00, 0x00];
        assert_eq!(sniff(pk, "xlsx"), Format::Xlsx);
        assert_eq!(sniff(pk, "docx"), Format::Docx);
        assert_eq!(sniff(pk, "zip"), Format::ArchiveZip);
    }

    #[test]
    fn textish_ext_sniff() {
        assert_eq!(sniff(b"", "yaml"), Format::Code);
        assert_eq!(sniff(b"", "toml"), Format::Code);
        assert_eq!(sniff(b"", "ics"), Format::Ics);
        assert_eq!(sniff(b"", "tsx"), Format::Code);
    }

    #[test]
    fn aif_and_aiff_route_to_audio() {
        assert!(is_audio_ext("aif"), "`aif` must route to the audio path");
        assert!(is_audio_ext("aiff"), "`aiff` must route to the audio path");
        assert_eq!(sniff_ext("aif"), Format::Audio);
        assert_eq!(sniff_ext("aiff"), Format::Audio);
        assert_eq!(sniff_ext("wma"), Format::Audio, "wma must remain in the audio path so the runtime chooser (not the text viewer) handles it");
    }

    #[test]
    fn wma_does_not_route_to_video() {
        assert!(!is_video_ext("wma"), "wma must not be classified as video — it goes through the audio media viewer and the runtime chooser");
    }

    #[test]
    fn iwork_extensions_route_to_iwork_not_archive() {
        let pk = &[0x50, 0x4B, 0x03, 0x04, 0x00, 0x00];
        assert_eq!(sniff(pk, "pages"), Format::IworkPages);
        assert_eq!(sniff(pk, "numbers"), Format::IworkNumbers);
        assert_eq!(sniff(pk, "key"), Format::IworkKey);
        // ext_from_sniff must round-trip to the iWork extension (not "zip")
        // so the Android URI fast-path treats iWork as a semantic document.
        assert_eq!(ext_from_sniff(pk, "pages").unwrap(), "pages");
        assert_eq!(ext_from_sniff(pk, "numbers").unwrap(), "numbers");
        assert_eq!(ext_from_sniff(pk, "key").unwrap(), "key");
    }

    #[test]
    fn legacy_ppt_routes_to_ppt_not_pptx() {
        assert_eq!(sniff_ext("ppt"), Format::Ppt);
    }
}
