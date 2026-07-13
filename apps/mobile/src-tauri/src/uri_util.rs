//! URI reads via tauri-plugin-fs (content:// + file://).

use std::str::FromStr;

use tauri::AppHandle;
use tauri_plugin_fs::{FilePath, FsExt};
use viewit_core::{open, reject_if_too_large, Document, Suggestion, OPEN_BYTES_CAP};
use viewit_core_types::Format as F;

fn is_video_ext(ext: &str) -> bool {
    matches!(
        ext,
        "mp4" | "m4v" | "webm" | "mkv" | "mov" | "avi" | "mpg" | "mpeg" | "3gp" | "wmv" | "flv"
    )
}

pub fn uri_display_name(uri: &str, name: Option<String>) -> (String, String) {
    if let Some(n) = name.filter(|s| !s.is_empty()) {
        let ext = n.rsplit('.').next().unwrap_or("").to_lowercase();
        return (ext, n);
    }
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
                    .map(|s| std::path::PathBuf::from(s))
            })
        });
    let display_name = path_hint
        .as_ref()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .unwrap_or("file")
        .to_string();
    let ext = display_name
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_lowercase();
    (ext, display_name)
}

pub fn read_uri_bytes(app: &AppHandle, uri: &str) -> Result<Vec<u8>, String> {
    app.fs()
        .read(FilePath::from_str(uri).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

/// Extension-only gate (no disk read) — instant reject for video share intents.
pub fn probe_before_read(_app: &AppHandle, uri: &str, name: Option<String>) -> Result<Option<Document>, String> {
    let (ext, _) = uri_display_name(uri, name);
    if is_video_ext(&ext) {
        return Ok(Some(Document::Unsupported {
            format: F::Unsupported,
            reason: format!(
                "Video (.{}) — ViewIt doesn't play video. Use your gallery or player app.",
                ext
            ),
            suggestion: Suggestion::None,
        }));
    }
    Ok(None)
}

pub fn read_uri_meta(
    app: &AppHandle,
    uri: &str,
    name: Option<String>,
) -> Result<(Vec<u8>, String, String), String> {
    let (ext, display_name) = uri_display_name(uri, name);
    if is_video_ext(&ext) {
        return Err(format!("Video (.{}) not supported.", ext));
    }
    let bytes = read_uri_bytes(app, uri)?;
    if let Some(doc) = reject_if_too_large(bytes.len()) {
        return Err(match doc {
            Document::Unsupported { reason, .. } => reason,
            _ => "too large".into(),
        });
    }
    Ok((bytes, ext, display_name))
}

pub fn open_from_uri(
    app: &AppHandle,
    uri: String,
    name: Option<String>,
) -> Result<Document, String> {
    if let Some(doc) = probe_before_read(app, &uri, name.clone())? {
        return Ok(doc);
    }
    let (bytes, ext, display_name) = read_uri_meta(app, &uri, name)?;
    open(&bytes, &ext, &display_name).map_err(|e| e.to_string())
}

pub fn open_from_bytes(bytes: Vec<u8>, name: String) -> Result<Document, String> {
    if let Some(doc) = reject_if_too_large(bytes.len()) {
        return Ok(doc);
    }
    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
    if is_video_ext(&ext) {
        return Ok(Document::Unsupported {
            format: F::Unsupported,
            reason: "Video not supported in ViewIt.".into(),
            suggestion: Suggestion::None,
        });
    }
    let display = if name.is_empty() { "file".to_string() } else { name };
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