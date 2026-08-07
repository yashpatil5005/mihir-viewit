//! URI reads via tauri-plugin-fs (content:// + file://).
//!
//! New streaming architecture:
//! - Read only 256KB for format sniffing
//! - Register URI with stream server for on-demand content access
//! - Return Document with stream_url for frontend HTTP fetching

use std::str::FromStr;

use tauri::{AppHandle, Manager};
use tauri_plugin_fs::{FilePath, FsExt};
use viewit_core::{
    ext_from_sniff, ext_hint_from_magic, is_audio_ext, is_stream_ext, is_video_ext, open,
    reject_if_too_large, sniff, Document, Format, OPEN_BYTES_CAP,
};
use viewit_core_types::MediaKind;

use crate::materialize;
use crate::stream_server;

const SNIFF_READ_CAP: usize = 256 * 1024;

fn is_image_format(format: Format) -> bool {
    matches!(
        format,
        Format::ImagePng
            | Format::ImageJpg
            | Format::ImageWebp
            | Format::ImageGif
            | Format::ImageBmp
            | Format::ImageTiff
            | Format::ImageSvg
            | Format::ImageHeic
            | Format::ImagePsd
            | Format::ImageRaw
    )
}

fn ext_hint_from_android_uri(uri: &str) -> Option<String> {
    let lower = uri.to_lowercase();
    if lower.contains("video%3a") || lower.contains("video:") || lower.contains("/video/") {
        return Some("mp4".into());
    }
    if lower.contains("audio%3a") || lower.contains("audio:") || lower.contains("/audio/") {
        return Some("mp3".into());
    }
    None
}

fn ext_from_mime(mime: &str) -> Option<&'static str> {
    let m = mime.trim().to_lowercase();
    if m.starts_with("video/") {
        return Some(match m.as_str() {
            "video/mp4" | "video/x-m4v" | "video/quicktime" => "mp4",
            "video/x-flv" => "flv",
            "video/x-msvideo" | "video/avi" => "avi",
            "video/x-matroska" => "mkv",
            "video/webm" => "webm",
            "video/mpeg" => "mpeg",
            "video/mp2t" | "video/mp2ts" => "ts",
            "video/3gpp" => "3gp",
            "video/x-ms-asf" => "asf",
            "video/ogg" => "ogv",
            "video/x-ms-wmv" => "wmv",
            "video/x-f4v" => "f4v",
            _ => "mp4",
        });
    }
    if m.starts_with("audio/") {
        return Some(match m.as_str() {
            "audio/mpeg" | "audio/mp3" => "mp3",
            "audio/x-wav" | "audio/wav" => "wav",
            "audio/ogg" => "ogg",
            "audio/flac" => "flac",
            "audio/aac" => "aac",
            "audio/x-m4a" | "audio/mp4" => "m4a",
            "audio/webm" => "opus",
            "audio/aiff" | "audio/x-aiff" => "aif",
            "audio/x-flac" => "flac",
            "audio/x-opus" | "audio/opus" => "opus",
            "audio/x-ms-wma" | "audio/wma" => "wma",
            _ => "mp3",
        });
    }
    if m.starts_with("image/") {
        return Some(match m.as_str() {
            "image/png" => "png",
            "image/jpeg" | "image/jpg" => "jpg",
            "image/gif" => "gif",
            "image/webp" => "webp",
            "image/bmp" => "bmp",
            "image/tiff" | "image/tif" => "tiff",
            "image/svg+xml" => "svg",
            "image/heic" | "image/heic-sequence" | "image/heif" | "image/heif-sequence" => "heic",
            "image/avif" | "image/avis" => "avif",
            "image/x-adobe-dng" | "image/dng" => "dng",
            "image/x-photoshop" | "image/vnd.adobe.photoshop" => "psd",
            "image/x-nikon-nef" => "nef",
            "image/x-canon-cr2" | "image/x-canon-cr3" => "cr2",
            "image/x-sony-arw" => "arw",
            "image/x-olympus-orf" => "orf",
            "image/x-fuji-raf" => "raf",
            "image/x-panasonic-rw2" => "rw2",
            "image/x-samsung-srw" => "srw",
            "image/x-pentax-pef" => "pef",
            _ => "png",
        });
    }
    if m.starts_with("application/") {
        return Some(match m.as_str() {
            "application/pdf" => "pdf",
            "application/msword" | "application/vnd.ms-word" => "doc",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => "docx",
            "application/vnd.ms-excel" | "application/vnd.ms-powerpoint" => "xls",
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => "xlsx",
            "application/vnd.openxmlformats-officedocument.presentationml.presentation" => "pptx",
            "application/vnd.oasis.opendocument.text" => "odt",
            "application/vnd.oasis.opendocument.spreadsheet" => "ods",
            "application/vnd.oasis.opendocument.presentation" => "odp",
            "application/epub+zip" => "epub",
            "application/x-mobipocket-ebook" => "mobi",
            "application/zip" => "zip",
            "application/x-tar" => "tar",
            "application/gzip" | "application/x-gzip" => "gz",
            "application/rtf" => "rtf",
            "application/json" => "json",
            "application/xml" | "application/xhtml+xml" => "xml",
            "application/javascript" => "js",
            _ => "txt",
        });
    }
    if m.starts_with("font/") {
        return Some(match m.as_str() {
            "font/otf" | "font/ttf" | "font/woff" | "font/woff2" => "otf",
            _ => "ttf",
        });
    }
    if m.starts_with("text/") {
        return Some(match m.as_str() {
            "text/css" => "css",
            "text/html" => "html",
            "text/csv" => "csv",
            "text/markdown" => "md",
            "text/xml" => "xml",
            "text/javascript" | "text/typescript" => "js",
            _ => "txt",
        });
    }
    None
}

fn is_office_format(format: Format) -> bool {
    matches!(
        format,
        Format::Docx
            | Format::Xlsx
            | Format::Pptx
            | Format::Odt
            | Format::Ods
            | Format::Odp
            | Format::Doc
            | Format::Ppt
            | Format::Xls
    )
}

fn is_iwork_format(format: Format) -> bool {
    matches!(
        format,
        Format::IworkPages | Format::IworkNumbers | Format::IworkKey
    )
}

fn format_from_ext(ext: &str) -> Format {
    sniff(&[], ext)
}

fn sanitize_ext(ext: &str) -> String {
    let e = ext.trim().to_lowercase();
    if e.is_empty() || e.len() > 12 || !e.chars().all(|c| c.is_ascii_alphanumeric()) {
        return String::new();
    }
    e
}

pub fn uri_display_name(app: &AppHandle, uri: &str, name: Option<String>) -> (String, String) {
    if let Some(n) = name.filter(|s| !s.is_empty()) {
        let ext = sanitize_ext(n.rsplit('.').next().unwrap_or(""));
        return (ext, n);
    }

    // Check if we have a display name from the Kotlin MediaStore query
    #[cfg(target_os = "android")]
    if let Some(pending) = app.try_state::<crate::PendingDisplayNames>() {
        if let Some(display_name) = pending.0.lock().unwrap().get(uri) {
            let dn = display_name.clone();
            let ext = sanitize_ext(dn.rsplit('.').next().unwrap_or(""));
            return (ext, dn);
        }
    }

    if let Some(plugin_name) = app.path().file_name(uri) {
        if !plugin_name.is_empty() && plugin_name != "file" {
            let ext = sanitize_ext(plugin_name.rsplit('.').next().unwrap_or(""));
            return (ext, plugin_name);
        }
    }
    let decoded = percent_decode_path(uri);
    let url = tauri::Url::parse(uri).ok();
    let path_hint = url
        .as_ref()
        .and_then(|u| {
            if u.scheme() == "file" {
                u.to_file_path().ok()
            } else {
                None
            }
        })
        .or_else(|| {
            url.as_ref().and_then(|u| {
                u.path_segments()
                    .and_then(|s| s.last())
                    .map(|s| std::path::PathBuf::from(percent_decode_segment(s)))
            })
        });
    let mut display_name = path_hint
        .as_ref()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "file".into());
    if display_name == "file" || !display_name.contains('.') {
        if let Some(tail) = decoded
            .rsplit('/')
            .next()
            .filter(|s| s.contains('.') && s.len() < 200)
        {
            display_name = tail.to_string();
        }
    }
    let mut ext = sanitize_ext(display_name.rsplit('.').next().unwrap_or(""));
    if ext.is_empty() {
        if let Some(h) = ext_hint_from_android_uri(uri) {
            ext = h;
            if !display_name.contains('.') {
                display_name = format!("shared.{}", ext);
            }
        }
    }
    (ext, display_name)
}

fn percent_decode_segment(s: &str) -> String {
    let mut raw: Vec<u8> = Vec::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) =
                u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16)
            {
                raw.push(v);
                i += 3;
                continue;
            }
        }
        raw.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&raw).into_owned()
}

fn percent_decode_path(uri: &str) -> String {
    let path = uri.split("://").nth(1).unwrap_or(uri);
    percent_decode_segment(path)
}

/// Read only the first SNIFF_READ_CAP bytes for format detection.
pub fn read_uri_prefix(app: &AppHandle, uri: &str) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let fp = FilePath::from_str(uri).map_err(|e| e.to_string())?;
    let mut opts = tauri_plugin_fs::OpenOptions::new();
    opts.read(true);
    let reader = app
        .fs()
        .open(fp, opts)
        .map_err(|e| format!("failed to open {}: {}", uri, e))?;
    let mut buf = Vec::with_capacity(SNIFF_READ_CAP);
    reader
        .take(SNIFF_READ_CAP as u64)
        .read_to_end(&mut buf)
        .map_err(|e| e.to_string())?;
    Ok(buf)
}

/// Read full URI bytes (for small files only).
pub fn read_uri_bytes(app: &AppHandle, uri: &str) -> Result<Vec<u8>, String> {
    app.fs()
        .read(FilePath::from_str(uri).expect("infallible FilePath parse"))
        .map_err(|e| e.to_string())
}

/// Read full URI bytes via an open stream (not `.read()`).
/// On Android, `tauri-plugin-fs` `.read()` fails with EACCES on raw `file://`
/// paths that fall under scoped storage, while `.open()` (used by the stream
/// server) succeeds for the same paths. Use this for IPC byte reads.
pub fn read_uri_bytes_open(app: &AppHandle, uri: &str) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let fp = FilePath::from_str(uri).map_err(|e| e.to_string())?;
    let mut opts = tauri_plugin_fs::OpenOptions::new();
    opts.read(true);
    let mut reader = app
        .fs()
        .open(fp, opts)
        .map_err(|e| format!("failed to open {}: {}", uri, e))?;
    let mut buf = Vec::new();
    reader
        .read_to_end(&mut buf)
        .map_err(|e| format!("read error: {}", e))?;
    Ok(buf)
}

fn sniff_prefix(app: &AppHandle, uri: &str, ext: &str) -> Result<(String, Format), String> {
    let mut ext_hint = sanitize_ext(ext);

    // Check for MIME type hint from Android intent (warm-start path)
    #[cfg(target_os = "android")]
    if ext_hint.is_empty() || ext_hint.len() > 8 {
        if let Some(mime_state) = app.try_state::<crate::android_pending::PendingUrisWithMime>() {
            if let Some(mime) = mime_state.get_mime(uri) {
                if let Some(h) = ext_from_mime(&mime) {
                    ext_hint = h.to_string();
                }
            }
        }
    }

    if ext_hint.is_empty() {
        if let Some(h) = ext_hint_from_android_uri(uri) {
            ext_hint = h;
        }
    }
    if is_video_ext(&ext_hint) {
        return Ok((ext_hint.clone(), Format::Video));
    }
    if is_audio_ext(&ext_hint) {
        return Ok((ext_hint.clone(), Format::Audio));
    }
    if ext_hint == "pdf" {
        return Ok((ext_hint, Format::Pdf));
    }
    if !ext_hint.is_empty() {
        let f = format_from_ext(&ext_hint);
        if is_image_format(f) || is_office_format(f) || is_iwork_format(f) {
            return Ok((ext_hint.clone(), f));
        }
    }

    let buf = read_uri_prefix(app, uri)?;
    if buf.is_empty() {
        return Err("empty file".into());
    }
    let ext_guess = ext_from_sniff(&buf, &ext_hint)
        .or_else(|| ext_hint_from_magic(&buf))
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            if ext_hint.is_empty() {
                "txt".into()
            } else {
                ext_hint
            }
        });
    Ok((ext_guess.clone(), sniff(&buf, &ext_guess)))
}

pub fn probe_before_read(
    _app: &AppHandle,
    uri: &str,
    name: Option<String>,
) -> Result<Option<Document>, String> {
    let _ = (uri, name);
    Ok(None)
}

pub fn read_uri_meta(
    app: &AppHandle,
    uri: &str,
    name: Option<String>,
) -> Result<(Vec<u8>, String, String), String> {
    let (ext, display_name) = uri_display_name(app, uri, name);
    let (ext, _format) = sniff_prefix(app, uri, &ext)?;
    let bytes = read_uri_bytes(app, uri)?;
    if let Some(doc) = reject_if_too_large(bytes.len()) {
        return Err(match doc {
            Document::Unsupported { reason, .. } => reason,
            _ => "too large".into(),
        });
    }
    Ok((bytes, ext, display_name))
}

/// Build a stream URL for the given URI.
fn build_stream_url(app: &AppHandle, uri: &str) -> Option<String> {
    let registry = app.state::<stream_server::StreamRegistry>();
    let id = stream_server::register_uri(&registry, uri.to_string());
    let port = app
        .state::<crate::HttpPort>()
        .0
        .load(std::sync::atomic::Ordering::Relaxed);
    if port > 0 {
        Some(format!("http://127.0.0.1:{}/{}", port, id))
    } else {
        None
    }
}

/// Materialize a shared (content:// / file://) file into app-private cache and
/// register it for streaming from cache via `std::fs` (Range-supported).
///
/// This is the scoped-storage workaround: raw `file:///sdcard/...` reads through
/// the plugin-fs/stream path return EACCES once MANAGE_EXTERNAL_STORAGE is not
/// granted (Android auto-revokes it), which made every media/pdf/image format
/// break with `TypeError: Failed to fetch` / `code=4`. Copying into app cache at
/// open time makes streaming + IPC byte reads permission-independent.
fn materialize_cached_stream(
    app: &AppHandle,
    uri: &str,
    ext: &str,
) -> Result<(Option<String>, String), String> {
    let (path, _size) = materialize::materialize_uri_to_cache(app, uri, ext)?;
    let asset_path = materialize::path_to_file_url(&path)?;
    let stream_url = crate::stream_server::register_cached_path(app, path, ext.to_string());
    let stream_url = if stream_url.is_empty() {
        None
    } else {
        Some(stream_url)
    };
    Ok((stream_url, asset_path))
}

pub fn open_from_uri(
    app: &AppHandle,
    uri: String,
    name: Option<String>,
    mime_type: Option<String>,
) -> Result<Document, String> {
    let (mut ext, display_name) = uri_display_name(app, &uri, name);

    // If ext is empty, try to derive from the provided MIME type
    if ext.is_empty() {
        if let Some(mime) = &mime_type {
            if let Some(h) = ext_from_mime(mime) {
                ext = h.to_string();
            }
        }
    }

    let (ext, format) = sniff_prefix(app, &uri, &ext)?;

    // PDF: materialize to app cache, register cached stream; read bytes over IPC
    if format == Format::Pdf && (uri.starts_with("content://") || uri.starts_with("file://")) {
        let (stream_url, asset_path) = match materialize_cached_stream(app, &uri, "pdf") {
            Ok(v) => v,
            Err(e) => {
                eprintln!(
                    "[viewit] pdf materialize failed, falling back to raw stream: {}",
                    e
                );
                let stream_url = build_stream_url(app, &uri);
                (stream_url, String::new())
            }
        };
        return Ok(Document::Pdf {
            page_count: 0,
            pages: vec![],
            byte_len: 0,
            native: true,
            name: display_name,
            asset_path,
            stream_url,
        });
    }

    // Video/Audio: register stream, return Media with stream_url
    if format == Format::Video
        || format == Format::Audio
        || is_video_ext(&ext)
        || is_audio_ext(&ext)
        || is_stream_ext(&ext)
    {
        let kind = if format == Format::Audio || is_audio_ext(&ext) {
            MediaKind::Audio
        } else {
            MediaKind::Video
        };
        let media_ext = if ext.is_empty() {
            if kind == MediaKind::Audio {
                "mp3".to_string()
            } else {
                "mp4".to_string()
            }
        } else {
            ext.clone()
        };
        let do_cache = uri.starts_with("content://") || uri.starts_with("file://");
        let (stream_url, asset_path) = if do_cache {
            match materialize_cached_stream(app, &uri, &media_ext) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!(
                        "[viewit] media materialize failed, falling back to raw stream: {}",
                        e
                    );
                    let stream_url = build_stream_url(app, &uri);
                    (stream_url, String::new())
                }
            }
        } else {
            let stream_url = build_stream_url(app, &uri);
            (stream_url, String::new())
        };
        return Ok(Document::Media {
            format,
            media_kind: kind,
            name: display_name,
            byte_len: 0,
            asset_path,
            ext: ext.clone(),
            stream_url,
        });
    }

    // Images: register stream, return Image with stream_url
    if is_image_format(format) {
        if uri.starts_with("content://") || uri.starts_with("file://") {
            // Convert unsupported formats (TIFF, DNG, NEF, PSD) to PNG for WebView
            if viewit_fmt_image::needs_conversion(format) {
                let bytes = read_uri_bytes(app, &uri)?;
                match viewit_fmt_image::decode_to_png(&bytes, format) {
                    Ok(png_bytes) => {
                        let cache_dir = app.path().app_cache_dir().map_err(|e| e.to_string())?;
                        std::fs::create_dir_all(&cache_dir).map_err(|e| e.to_string())?;
                        let cache_path = cache_dir.join(format!("{}_converted.png", display_name));
                        std::fs::write(&cache_path, &png_bytes).map_err(|e| e.to_string())?;
                        let stream_url = crate::stream_server::register_cached_path(
                            app,
                            cache_path,
                            "png".into(),
                        );
                        return Ok(Document::Image {
                            format,
                            byte_len: png_bytes.len(),
                            name: display_name,
                            asset_path: String::new(),
                            stream_url: Some(stream_url),
                        });
                    }
                    Err(e) => {
                        eprintln!("[viewit] failed to convert {:?} to PNG: {}", format, e);
                    }
                }
            }
            let stream_url = build_stream_url(app, &uri);
            return Ok(Document::Image {
                format,
                byte_len: 0,
                name: display_name,
                asset_path: String::new(),
                stream_url,
            });
        }
    }

    // PPTX: materialize (client-side viewer needs local file), then register stream
    let ext_l = ext.to_lowercase();
    if (format == Format::Pptx || ext_l == "pptx" || ext_l == "pptm" || ext_l == "potx")
        && (uri.starts_with("content://") || uri.starts_with("file://"))
    {
        let use_ext = if ext_l.is_empty() {
            "pptx"
        } else {
            ext_l.as_str()
        };
        let doc = materialize::open_pptx_materialized(app, &uri, &display_name, use_ext)?;
        if let Document::Pptx { asset_path, .. } = &doc {
            if !asset_path.is_empty() {
                // Also register for streaming
                let stream_url = build_stream_url(app, &uri);
                if let Document::Pptx {
                    stream_url: ref mut _su,
                    ..
                } = doc.clone()
                {
                    // Reconstruct with stream_url
                    return Ok(Document::Pptx {
                        slide_count: 0,
                        slides: vec![],
                        byte_len: 0,
                        asset_path: asset_path.clone(),
                        stream_url,
                    });
                }
            }
        }
        return Ok(doc);
    }

    // Office/iWork files (non-PPTX): read full bytes (they need full content for parsing)
    if is_office_format(format) || is_iwork_format(format) {
        let bytes = read_uri_bytes(app, &uri)?;
        if bytes.len() > OPEN_BYTES_CAP {
            return Err(format!(
                "Document is {:.1} MB — max {} MB in memory.",
                bytes.len() as f64 / 1_048_576.0,
                OPEN_BYTES_CAP / 1_048_576
            ));
        }
        return open(&bytes, &ext, &display_name).map_err(|e| e.to_string());
    }

    // Text-like files: read full bytes for parsing (text is usually small)
    let bytes = read_uri_bytes(app, &uri)?;
    if bytes.len() > OPEN_BYTES_CAP {
        return Err(format!(
            "File is {:.1} MB — max {} MB in memory.",
            bytes.len() as f64 / 1_048_576.0,
            OPEN_BYTES_CAP / 1_048_576
        ));
    }
    open(&bytes, &ext, &display_name).map_err(|e| e.to_string())
}

pub fn open_from_bytes(bytes: Vec<u8>, name: String) -> Result<Document, String> {
    let mut ext = sanitize_ext(name.rsplit('.').next().unwrap_or(""));
    if ext.is_empty() {
        ext = ext_hint_from_magic(&bytes).unwrap_or("txt").to_string();
    }
    let display = if name.is_empty() {
        "file".to_string()
    } else {
        name.clone()
    };

    if is_stream_ext(&ext) && bytes.len() > OPEN_BYTES_CAP {
        return Err("Large PDF/video: use Open with or the in-app file picker.".into());
    }
    if (is_video_ext(&ext) || is_audio_ext(&ext)) && bytes.len() > OPEN_BYTES_CAP {
        return Err("Video/audio is too large via Share — use Open with ViewIt.".into());
    }

    if let Some(doc) = reject_if_too_large(bytes.len()) {
        return Ok(doc);
    }
    open(&bytes, &ext, &display).map_err(|e| e.to_string())
}

#[cfg(feature = "fmt-pdf")]
pub fn pdf_page_from_uri(app: &AppHandle, uri: String, index: usize) -> Result<String, String> {
    let bytes = read_uri_bytes(app, &uri)?;
    if bytes.len() > OPEN_BYTES_CAP {
        return Err("PDF too large for in-app paging".into());
    }
    viewit_fmt_pdf::render_page(&bytes, index).map_err(|e| e.to_string())
}
