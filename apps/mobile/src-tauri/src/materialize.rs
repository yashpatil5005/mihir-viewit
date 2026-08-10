//! Copy content:// to app cache for convertFileSrc (Android custom protocol fails).

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use tauri::{AppHandle, Manager};
use tauri_plugin_fs::{FilePath, FsExt};
use viewit_core_types::Document;

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

pub fn materialize_uri_to_cache(
    app: &AppHandle,
    uri: &str,
    ext: &str,
) -> Result<(PathBuf, usize), String> {
    let dir = cache_dir(app)?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let dest = dir.join(stable_cache_name(uri, ext));
    // Always delete stale cache — content:// URIs can be reused by Android
    // to point to different files, so we must re-materialize on every open.
    if dest.exists() {
        let _ = std::fs::remove_file(&dest);
    }
    let fp = FilePath::from_str(uri).expect("infallible FilePath parse");
    let mut opts = tauri_plugin_fs::OpenOptions::new();
    opts.read(true);
    let mut reader = app.fs().open(fp, opts).map_err(|e| e.to_string())?;
    let mut file = std::fs::File::create(&dest).map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; 128 * 1024];
    let mut total = 0usize;
    loop {
        let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        total += n;
        if total > MAX_MATERIALIZE_BYTES {
            let _ = std::fs::remove_file(&dest);
            return Err(format!(
                "File is {:.1} MB — max {:.0} MB for in-app PDF/video.",
                total as f64 / 1_048_576.0,
                MAX_MATERIALIZE_BYTES as f64 / 1_048_576.0
            ));
        }
        file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
    }
    let _ = prune_viewit_cache(app);
    Ok((dest, total))
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
    url::Url::from_file_path(path)
        .map_err(|_| "bad cache path".to_string())
        .map(|u| u.to_string())
}

pub fn open_pptx_materialized(
    app: &AppHandle,
    uri: &str,
    display_name: &str,
    ext: &str,
) -> Result<Document, String> {
    let (path, size) = materialize_uri_to_cache(app, uri, ext)?;
    let asset_path = path_to_file_url(&path)?;

    // Basic offline preview: parse native text slides via the baked-in fmt-office
    // so the built-in viewer has something to show even with no plugin/WASM. The
    // faithful renderer (office-universal + pptx-vanilla) still re-renders from
    // asset_path when installed.
    let (slide_count, slides, parsed_len) = std::fs::read(&path)
        .ok()
        .and_then(|bytes| viewit_core::open(&bytes, "pptx", display_name).ok())
        .and_then(|doc| match doc {
            Document::Pptx { slide_count, slides, byte_len, .. } => Some((slide_count, slides, byte_len)),
            _ => None,
        })
        .unwrap_or((0, Vec::new(), size));

    Ok(Document::Pptx {
        slide_count,
        slides,
        byte_len: if parsed_len > 0 { parsed_len } else { size },
        asset_path,
        stream_url: None,
    })
}
