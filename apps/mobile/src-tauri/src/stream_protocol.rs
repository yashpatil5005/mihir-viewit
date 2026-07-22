//! Stream content:// / file:// to WebView via custom protocol (no RAM in JS).

use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use http::header::{ACCEPT_RANGES, CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, RANGE};
use http::{Request, Response, StatusCode};
use mime_guess::from_path;
use tauri::{AppHandle, Manager};
use tauri_plugin_fs::{FilePath, FsExt};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone)]
pub struct StreamEntry {
    pub source_uri: String,
    pub cache_path: Option<PathBuf>,
    pub ext_hint: String,
}

#[derive(Default)]
pub struct StreamSlots(pub Mutex<HashMap<u64, StreamEntry>>);

pub fn insert_uri(slots: &StreamSlots, uri: String) -> u64 {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let ext = uri
        .rsplit('.')
        .next()
        .unwrap_or("")
        .split('?')
        .next()
        .unwrap_or("")
        .to_lowercase();
    let ext_hint = if ext.is_empty() {
        let lower = uri.to_lowercase();
        if lower.contains("video") {
            "mp4".into()
        } else if lower.contains("audio") {
            "mp3".into()
        } else {
            String::new()
        }
    } else {
        ext
    };
    slots.0.lock().unwrap().insert(
        id,
        StreamEntry {
            source_uri: uri,
            cache_path: None,
            ext_hint,
        },
    );
    id
}

/// Register an already-materialized file (in cache) with the stream protocol.
/// `cache_path` is the on-disk file; `ext_hint` sets the MIME type.
/// Returns the stream ID.
pub fn insert_cached(slots: &StreamSlots, cache_path: PathBuf, ext_hint: String) -> u64 {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    slots.0.lock().unwrap().insert(
        id,
        StreamEntry {
            source_uri: cache_path.to_string_lossy().to_string(),
            cache_path: Some(cache_path),
            ext_hint,
        },
    );
    id
}

pub fn guess_mime(uri: &str, ext_hint: &str) -> String {
    if !ext_hint.is_empty() {
        if let Some(m) = from_path(format!("file.{}", ext_hint)).first() {
            return m.essence_str().to_string();
        }
    }
    if uri.contains(".pdf") {
        return "application/pdf".into();
    }
    if uri.to_lowercase().contains("video") || uri.contains(".mp4") || uri.contains(".m4v") {
        return "video/mp4".into();
    }
    if uri.contains(".webm") {
        return "video/webm".into();
    }
    if uri.contains(".mp3") {
        return "audio/mpeg".into();
    }
    "application/octet-stream".into()
}

pub fn parse_range_header(range: &str, len: usize) -> Option<(usize, usize)> {
    let s = range.strip_prefix("bytes=")?;
    let (start_s, end_s) = s.split_once('-')?;
    let start: usize = start_s.parse().ok()?;
    let end = if end_s.is_empty() {
        len.saturating_sub(1)
    } else {
        end_s.parse().ok()?
    };
    if start >= len || end >= len || start > end {
        return None;
    }
    Some((start, end))
}

pub fn ensure_cached(app: &AppHandle, entry: &mut StreamEntry) -> Result<(PathBuf, usize), String> {
    if let Some(ref p) = entry.cache_path {
        if p.exists() {
            let len = std::fs::metadata(p).map_err(|e| e.to_string())?.len() as usize;
            return Ok((p.clone(), len));
        }
    }
    let fp = FilePath::from_str(&entry.source_uri).expect("infallible FilePath parse");
    let bytes = app.fs().read(fp).map_err(|e| e.to_string())?;
    let dir = app.path().app_cache_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let safe_ext = if entry.ext_hint.is_empty() {
        "bin"
    } else {
        &entry.ext_hint
    };
    let dest = dir.join(format!(
        "stream_{}_{}.{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
        entry.source_uri.len().min(8),
        safe_ext
    ));
    std::fs::write(&dest, &bytes).map_err(|e| e.to_string())?;
    let len = bytes.len();
    entry.cache_path = Some(dest.clone());
    Ok((dest, len))
}

pub fn read_file_range(path: &PathBuf, start: usize, end: usize) -> Result<Vec<u8>, String> {
    let mut f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    f.seek(SeekFrom::Start(start as u64))
        .map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; end - start + 1];
    f.read_exact(&mut buf).map_err(|e| e.to_string())?;
    Ok(buf)
}

pub fn handle_stream_request_http(
    app: &AppHandle,
    slots: &StreamSlots,
    request: Request<Vec<u8>>,
) -> Response<Vec<u8>> {
    let req_uri = request.uri().to_string();
    let path = req_uri
        .strip_prefix("viewit-stream://localhost/")
        .or_else(|| {
            req_uri
                .strip_prefix("https://viewit-stream.localhost/")
                .map(|p| p.split('?').next().unwrap_or(p))
        })
        .unwrap_or(&req_uri);
    let id: u64 = match path.split('/').next().and_then(|s| s.parse().ok()) {
        Some(id) => id,
        None => {
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(b"bad stream id".to_vec())
                .unwrap();
        }
    };
    let mut entry = {
        let map = slots.0.lock().unwrap();
        match map.get(&id) {
            Some(e) => StreamEntry {
                source_uri: e.source_uri.clone(),
                cache_path: e.cache_path.clone(),
                ext_hint: e.ext_hint.clone(),
            },
            None => {
                return Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(b"stream slot expired".to_vec())
                    .unwrap();
            }
        }
    };

    let cached = match ensure_cached(app, &mut entry) {
        Ok(v) => v,
        Err(e) => {
            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(format!("cache failed: {}", e).into_bytes())
                .unwrap();
        }
    };
    {
        let mut map = slots.0.lock().unwrap();
        if let Some(e) = map.get_mut(&id) {
            e.cache_path = entry.cache_path;
        }
    }

    let (cache_path, len) = cached;
    let mime = guess_mime(&entry.source_uri, &entry.ext_hint);

    if let Some(range_val) = request.headers().get(RANGE).and_then(|v| v.to_str().ok()) {
        if let Some((start, end)) = parse_range_header(range_val, len) {
            match read_file_range(&cache_path, start, end) {
                Ok(slice) => {
                    let content_range = format!("bytes {}-{}/{}", start, end, len);
                    return Response::builder()
                        .status(StatusCode::PARTIAL_CONTENT)
                        .header(CONTENT_TYPE, mime)
                        .header(CONTENT_LENGTH, slice.len())
                        .header(CONTENT_RANGE, content_range)
                        .header(ACCEPT_RANGES, "bytes")
                        .body(slice)
                        .unwrap();
                }
                Err(e) => {
                    return Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(format!("range read: {}", e).into_bytes())
                        .unwrap();
                }
            }
        }
    }

    match std::fs::read(&cache_path) {
        Ok(bytes) => Response::builder()
            .status(StatusCode::OK)
            .header(CONTENT_TYPE, mime)
            .header(CONTENT_LENGTH, len)
            .header(ACCEPT_RANGES, "bytes")
            .body(bytes)
            .unwrap(),
        Err(e) => Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(format!("read failed: {}", e).into_bytes())
            .unwrap(),
    }
}