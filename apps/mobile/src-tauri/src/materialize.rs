//! Copy content:// to app cache for convertFileSrc (Android custom protocol fails).

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use tauri::{AppHandle, Manager};
use tauri_plugin_fs::{FilePath, FsExt};
use viewit_core_types::{Document, Format, MediaKind, StreamKind};

pub const MAX_MATERIALIZE_BYTES: usize = 400 * 1024 * 1024;
const MAX_CACHE_FILES: usize = 6;

fn stable_cache_name(uri: &str, ext: &str) -> String {
    let mut h = DefaultHasher::new();
    uri.hash(&mut h);
    let safe_ext = if ext.is_empty() { "bin" } else { ext };
    format!("v_{:016x}.{}", h.finish(), safe_ext)
}

pub fn prune_viewit_cache(app: &AppHandle) -> Result<(), String> {
    let dir = cache_dir(app)?;
    if !dir.exists() {
        return Ok(());
    }
    let mut entries: Vec<_> = std::fs::read_dir(&dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_str()
                .map(|n| n.starts_with("v_"))
                .unwrap_or(false)
        })
        .collect();
    if entries.len() <= MAX_CACHE_FILES {
        return Ok(());
    }
    entries.sort_by_key(|e| e.metadata().and_then(|m| m.modified()).ok());
    let drop = entries.len().saturating_sub(MAX_CACHE_FILES);
    for e in entries.into_iter().take(drop) {
        let _ = std::fs::remove_file(e.path());
    }
    Ok(())
}

pub fn cache_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_cache_dir().map_err(|e| e.to_string())
}

pub fn materialize_uri_to_cache(app: &AppHandle, uri: &str, ext: &str) -> Result<(PathBuf, usize), String> {
    let dir = cache_dir(app)?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let dest = dir.join(stable_cache_name(uri, ext));
    // If cache exists, verify it's a real file (not 0 bytes from interrupted write).
    if dest.exists() {
        let meta = std::fs::metadata(&dest).map_err(|e| e.to_string())?;
        let len = meta.len() as usize;
        if len > 0 {
            return Ok((dest, len));
        }
        // 0-byte file = interrupted previous write; delete and re-materialize.
        let _ = std::fs::remove_file(&dest);
    }
    let fp = FilePath::from_str(uri).expect("infallible FilePath parse");
    let bytes = app.fs().read(fp).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_MATERIALIZE_BYTES {
        return Err(format!(
            "File is {:.1} MB — max {:.0} MB for in-app PDF/video.",
            bytes.len() as f64 / 1_048_576.0,
            MAX_MATERIALIZE_BYTES as f64 / 1_048_576.0
        ));
    }
    let mut file = std::fs::File::create(&dest).map_err(|e| e.to_string())?;
    file.write_all(&bytes).map_err(|e| e.to_string())?;
    let _ = file.sync_all();
    let _ = prune_viewit_cache(app);
    Ok((dest, bytes.len()))
}

pub fn read_materialized_file(app: &AppHandle, asset_path: &str) -> Result<Vec<u8>, String> {
    let path = if asset_path.starts_with("file://") {
        url::Url::parse(asset_path)
            .map_err(|e| e.to_string())?
            .to_file_path()
            .map_err(|_| "bad file url".to_string())?
    } else {
        PathBuf::from(asset_path)
    };
    let dir = cache_dir(app)?;
    if !path.starts_with(&dir) {
        return Err("path outside app cache".into());
    }
    std::fs::read(&path).map_err(|e| e.to_string())
}

pub fn path_to_file_url(path: &Path) -> Result<String, String> {
    url::Url::from_file_path(path).map_err(|_| "bad cache path".to_string()).map(|u| u.to_string())
}

pub fn mime_for_ext(ext: &str) -> &'static str {
    match ext {
        "pdf" => "application/pdf",
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "mkv" => "video/x-matroska",
        "mov" => "video/quicktime",
        "3gp" => "video/3gpp",
        "avi" => "video/x-msvideo",
        "mp3" => "audio/mpeg",
        "m4a" => "audio/mp4",
        "wav" => "audio/wav",
        "ogg" => "audio/ogg",
        "opus" => "audio/opus",
        _ => "application/octet-stream",
    }
}

pub fn open_pdf_materialized(
    app: &AppHandle,
    uri: &str,
    display_name: &str,
    ext: &str,
) -> Result<Document, String> {
    let (path, size) = materialize_uri_to_cache(app, uri, ext)?;
    let asset_path = path_to_file_url(&path)?;
    Ok(Document::StreamFile {
        asset_path,
        mime: mime_for_ext("pdf").into(),
        name: display_name.to_string(),
        byte_len: size,
        stream_kind: StreamKind::Pdf,
    })
}

pub fn open_media_materialized(
    app: &AppHandle,
    uri: &str,
    display_name: &str,
    ext: &str,
    kind: MediaKind,
) -> Result<Document, String> {
    let format = match kind {
        MediaKind::Video => Format::Video,
        MediaKind::Audio => Format::Audio,
    };

    let (path, size) = materialize_uri_to_cache(app, uri, ext)?;
    let asset_path = path_to_file_url(&path)?;

    eprintln!(
        "[viewit] media materialized: ext={} kind={:?} size={} path={}",
        ext, kind, size, asset_path
    );
    #[cfg(target_os = "android")]
    unsafe {
        use std::ffi::CString;
        let tag = CString::new("viewit").unwrap_or_default();
        let msg = CString::new(format!(
            "[viewit] media materialized: ext={} kind={:?} size={} path={}",
            ext, kind, size, asset_path
        ))
        .unwrap_or_default();
        extern "C" {
            fn __android_log_write(prio: i32, tag: *const u8, text: *const u8) -> i32;
        }
        __android_log_write(4, tag.as_ptr(), msg.as_ptr());
    }

    Ok(Document::Media {
        format,
        media_kind: kind,
        name: display_name.to_string(),
        byte_len: size,
        asset_path,
    })
}

pub fn open_pptx_materialized(
    app: &AppHandle,
    uri: &str,
    _display_name: &str,
    ext: &str,
) -> Result<Document, String> {
    let (path, size) = materialize_uri_to_cache(app, uri, ext)?;
    let asset_path = path_to_file_url(&path)?;
    Ok(Document::Pptx {
        slide_count: 0,
        slides: vec![],
        byte_len: size,
        asset_path,
    })
}

pub fn open_image_materialized(
    app: &AppHandle,
    uri: &str,
    display_name: &str,
    ext: &str,
    format: Format,
) -> Result<Document, String> {
    let (path, size) = materialize_uri_to_cache(app, uri, ext)?;
    let asset_path = path_to_file_url(&path)?;
    Ok(Document::Image {
        format,
        byte_len: size,
        name: display_name.to_string(),
        asset_path,
    })
}