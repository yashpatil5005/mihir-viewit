//! Unified localhost HTTP stream server for ALL file types.
//!
//! Replaces the old stream_protocol + materialize double-read pattern.
//! Now streams directly from `content://` URIs without full-file buffering.

use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::net::TcpListener;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use mime_guess::from_path;
use tauri::{AppHandle, Manager};
use tauri_plugin_fs::{FilePath, FsExt};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// A registered stream entry — either a `content://` URI or a cached file path.
#[derive(Clone)]
pub struct StreamEntry {
    pub source_uri: String,
    pub cache_path: Option<PathBuf>,
    pub ext_hint: String,
    pub mime_override: Option<String>,
}

#[derive(Default)]
pub struct StreamRegistry(pub Mutex<HashMap<u64, StreamEntry>>);

/// Register a `content://` or `file://` URI for streaming.
/// Returns the stream ID.
pub fn register_uri(registry: &StreamRegistry, uri: String) -> u64 {
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
        } else if lower.contains("pdf") {
            "pdf".into()
        } else {
            String::new()
        }
    } else {
        ext
    };
    registry.0.lock().unwrap().insert(
        id,
        StreamEntry {
            source_uri: uri,
            cache_path: None,
            ext_hint,
            mime_override: None,
        },
    );
    id
}

/// Register an already-materialized cache file.
pub fn register_cached(registry: &StreamRegistry, cache_path: PathBuf, ext_hint: String) -> u64 {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    registry.0.lock().unwrap().insert(
        id,
        StreamEntry {
            source_uri: cache_path.to_string_lossy().to_string(),
            cache_path: Some(cache_path),
            ext_hint,
            mime_override: None,
        },
    );
    id
}

/// Register a cached file path via AppHandle (convenience wrapper).
pub fn register_cached_path(app: &AppHandle, cache_path: PathBuf, ext_hint: String) -> String {
    let registry = app.state::<StreamRegistry>();
    let id = register_cached(&registry, cache_path, ext_hint);
    let port = app
        .state::<crate::HttpPort>()
        .0
        .load(std::sync::atomic::Ordering::Relaxed);
    if port > 0 {
        format!("http://127.0.0.1:{}/{}", port, id)
    } else {
        String::new()
    }
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
    if uri.contains(".mkv") {
        return "video/x-matroska".into();
    }
    if uri.contains(".mp3") {
        return "audio/mpeg".into();
    }
    if uri.contains(".m4a") {
        return "audio/mp4".into();
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

/// Read file range from a path.
pub fn read_file_range(path: &PathBuf, start: usize, end: usize) -> Result<Vec<u8>, String> {
    let mut f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    f.seek(SeekFrom::Start(start as u64))
        .map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; end - start + 1];
    f.read_exact(&mut buf).map_err(|e| e.to_string())?;
    Ok(buf)
}

/// Open a stream from a `content://` URI via tauri-plugin-fs.
fn open_content_stream(app: &AppHandle, uri: &str) -> Result<Box<dyn Read + Send>, String> {
    let fp = FilePath::from_str(uri).map_err(|e| e.to_string())?;
    let mut opts = tauri_plugin_fs::OpenOptions::new();
    opts.read(true);
    let file = app
        .fs()
        .open(fp, opts)
        .map_err(|e| format!("failed to open {}: {}", uri, e))?;
    Ok(Box::new(file))
}

/// Build headers for a response.
fn build_headers(mime: &str, len: usize, extra: Option<(&str, &str)>) -> Vec<tiny_http::Header> {
    let mut headers = vec![
        tiny_http::Header::from_bytes(&b"Content-Type"[..], mime.as_bytes()).unwrap(),
        tiny_http::Header::from_bytes(&b"Content-Length"[..], len.to_string().as_bytes()).unwrap(),
        tiny_http::Header::from_bytes(&b"Accept-Ranges"[..], &b"bytes"[..]).unwrap(),
        tiny_http::Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap(),
        tiny_http::Header::from_bytes(
            &b"Access-Control-Allow-Methods"[..],
            &b"GET, HEAD, OPTIONS"[..],
        )
        .unwrap(),
        tiny_http::Header::from_bytes(&b"Access-Control-Allow-Headers"[..], &b"Range"[..]).unwrap(),
    ];
    if let Some((key, val)) = extra {
        headers.push(tiny_http::Header::from_bytes(key.as_bytes(), val.as_bytes()).unwrap());
    }
    headers
}

/// Build an error response that still carries CORS headers. Without
/// `Access-Control-Allow-Origin`, cross-origin JS `fetch(stream_url)` in the
/// WebView rejects with an opaque `TypeError: Failed to fetch` on any failure
/// (e.g. scoped-storage EACCES), hiding the real HTTP status.
fn error_response(status: u16, msg: &str) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    tiny_http::Response::new(
        status.into(),
        build_headers("text/plain", msg.len(), None),
        std::io::Cursor::new(msg.as_bytes().to_vec()),
        Some(msg.len()),
        None,
    )
}

/// Handle an HTTP request for a stream.
fn handle_request(
    app: &AppHandle,
    registry: &StreamRegistry,
    request: &tiny_http::Request,
    stream_id: u64,
) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    let entry = {
        let map = registry.0.lock().unwrap();
        match map.get(&stream_id) {
            Some(e) => e.clone(),
            None => {
                return error_response(404, "stream slot expired");
            }
        }
    };

    let mime = entry
        .mime_override
        .clone()
        .unwrap_or_else(|| guess_mime(&entry.source_uri, &entry.ext_hint));

    // If we have a cached file path, serve it directly with Range support
    if let Some(ref cache_path) = entry.cache_path {
        if cache_path.exists() {
            let len = std::fs::metadata(cache_path)
                .map(|m| m.len() as usize)
                .unwrap_or(0);

            // Check for Range header
            if let Some(range_val) = request
                .headers()
                .iter()
                .find(|h| h.field.as_str() == "range")
            {
                let range_str = range_val.value.as_str();
                if let Some((start, end)) = parse_range_header(range_str, len) {
                    match read_file_range(cache_path, start, end) {
                        Ok(slice) => {
                            let content_range = format!("bytes {}-{}/{}", start, end, len);
                            let slice_len = slice.len();
                            return tiny_http::Response::new(
                                206.into(),
                                build_headers(
                                    &mime,
                                    slice_len,
                                    Some(("Content-Range", &content_range)),
                                ),
                                std::io::Cursor::new(slice),
                                Some(slice_len),
                                None,
                            );
                        }
                        Err(e) => {
                            return error_response(500, &format!("range error: {}", e));
                        }
                    }
                }
            }

            // Full file response
            match std::fs::read(cache_path) {
                Ok(bytes) => {
                    let bytes_len = bytes.len();
                    return tiny_http::Response::new(
                        200.into(),
                        build_headers(&mime, bytes_len, None),
                        std::io::Cursor::new(bytes),
                        Some(bytes_len),
                        None,
                    );
                }
                Err(e) => {
                    return error_response(500, &format!("read error: {}", e));
                }
            }
        }
    }

    // No cache — stream directly from content:// URI
    match open_content_stream(app, &entry.source_uri) {
        Ok(mut reader) => {
            let mut buf = Vec::new();
            if let Err(e) = reader.read_to_end(&mut buf) {
                return tiny_http::Response::from_string(format!("read error: {}", e))
                    .with_status_code(500);
            }

            let len = buf.len();

            // Check for Range header
            if let Some(range_val) = request
                .headers()
                .iter()
                .find(|h| h.field.as_str() == "range")
            {
                let range_str = range_val.value.as_str();
                if let Some((start, end)) = parse_range_header(range_str, len) {
                    let content_range = format!("bytes {}-{}/{}", start, end, len);
                    let slice = buf[start..=end].to_vec();
                    let slice_len = slice.len();
                    return tiny_http::Response::new(
                        206.into(),
                        build_headers(&mime, slice_len, Some(("Content-Range", &content_range))),
                        std::io::Cursor::new(slice),
                        Some(slice_len),
                        None,
                    );
                }
            }

            return tiny_http::Response::new(
                200.into(),
                build_headers(&mime, len, None),
                std::io::Cursor::new(buf),
                Some(len),
                None,
            );
        }
        Err(e) => error_response(500, &format!("stream error: {}", e)),
    }
}

/// Start the localhost HTTP stream server.
/// Returns the port number.
pub fn start(app: AppHandle) -> Result<u16, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();

    std::thread::spawn(move || {
        let server = match tiny_http::Server::from_listener(listener, None) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[viewit] failed to create HTTP server: {}", e);
                return;
            }
        };

        eprintln!("[viewit] HTTP stream server on 127.0.0.1:{}", port);

        for request in server.incoming_requests() {
            let url = request.url().to_string();
            let path = url.split('?').next().unwrap_or(&url);
            let id_str = path.trim_start_matches('/');

            let stream_id: u64 = match id_str.parse() {
                Ok(id) => id,
                Err(_) => {
                    let _ = request.respond(error_response(400, "bad stream id"));
                    continue;
                }
            };

            let registry = app.state::<StreamRegistry>();
            let response = handle_request(&app, &registry, &request, stream_id);
            let _ = request.respond(response);
        }
    });

    Ok(port)
}
