//! ViewIt mobile Tauri backend. Mirrors desktop commands; Android uses
//! `tauri-plugin-fs` for `content://` URIs (share / file picker).
//!
//! New streaming architecture:
//! - `open_uri` returns Document with `stream_url` for all formats
//! - Frontend fetches content via HTTP from localhost stream server
//! - No full-file reads into memory, no base64 encoding

#[cfg(target_os = "android")]
mod android_pending;

mod materialize;
mod stream_protocol;
mod stream_server;
mod uri_util;

use std::io::Read;
use std::sync::Mutex;

use base64::{engine::general_purpose, Engine as _};
use tauri::{Manager, Url};
use viewit_core::Document;

#[derive(Default, serde::Serialize)]
struct OpenedUrls(Mutex<Vec<Url>>);

#[derive(Default)]
struct PendingDisplayNames(Mutex<std::collections::HashMap<String, String>>);

#[derive(Default)]
struct HttpPort(std::sync::atomic::AtomicU16);

#[tauri::command]
fn opened_urls(app: tauri::AppHandle) -> Vec<String> {
    #[cfg(target_os = "android")]
    {
        // Check if setup() already drained the pending file into OpenedUrls state.
        let cached: Vec<String> = {
            let state = app.state::<OpenedUrls>();
            let mut lock = state.0.lock().unwrap();
            lock.drain(..).map(|u| u.to_string()).collect()
        };
        if !cached.is_empty() {
            return cached;
        }
        // Warm-start: file may still exist (onNewIntent wrote it after setup).
        let entries = android_pending::drain_into_opened_urls(&app);
        entries.iter().map(|(uri, _)| uri.clone()).collect()
    }
    #[cfg(not(target_os = "android"))]
    {
        let state = app.state::<OpenedUrls>();
        let mut lock = state.0.lock().unwrap();
        lock.drain(..).map(|u| u.to_string()).collect()
    }
}

#[tauri::command]
async fn open_uri(
    app: tauri::AppHandle,
    uri: String,
    name: Option<String>,
    mime_type: Option<String>,
) -> Result<Document, String> {
    #[cfg(target_os = "android")]
    {
        // Read pending file for warm-start (app already running, onNewIntent fired)
        android_pending::read_pending_mimes(&app);
    }
    uri_util::open_from_uri(&app, uri, name, mime_type)
}

#[tauri::command]
async fn decode_heic_to_data_url(
    app: tauri::AppHandle,
    asset_path: String,
    uri: Option<String>,
) -> Result<String, String> {
    use std::io::Read;
    use std::str::FromStr;
    use tauri_plugin_fs::{FilePath, FsExt};
    let data = if !asset_path.is_empty() && !asset_path.starts_with("content://") {
        let fp = std::path::PathBuf::from(&asset_path);
        std::fs::read(&fp).map_err(|e| format!("read HEIC: {e}"))?
    } else if let Some(ref u) = uri {
        if u.starts_with("file://") {
            let path = u.strip_prefix("file://").unwrap_or(u);
            std::fs::read(path).map_err(|e| format!("read HEIC: {e}"))?
        } else {
            let fp = FilePath::from_str(u).map_err(|e| format!("parse URI: {e}"))?;
            let mut opts = tauri_plugin_fs::OpenOptions::new();
            opts.read(true);
            let mut reader = app
                .fs()
                .open(fp, opts)
                .map_err(|e| format!("open HEIC via ContentResolver: {e}"))?;
            let mut buf = Vec::new();
            reader
                .read_to_end(&mut buf)
                .map_err(|e| format!("read HEIC via ContentResolver: {e}"))?;
            buf
        }
    } else {
        return Err("no path or URI for HEIC file".into());
    };
    // HEIC/HEIF decode via the pure-Rust `hpvcd` crate (BSD-3-Clause OR
    // Apache-2.0) — display-ready 8-bit RGB, orientation/crop applied. Replaces
    // the former AGPL-3.0 `heic` crate with no copyleft in the dependency graph.
    let rgb = hpvcd::decode_heic_rgb8(&data).map_err(|e| format!("decode HEIC: {e}"))?;
    let img = image::RgbImage::from_raw(rgb.width, rgb.height, rgb.pixels)
        .ok_or("invalid HEIC pixel dimensions")?;
    let mut jpeg_buf = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(img)
        .write_to(&mut jpeg_buf, image::ImageFormat::Jpeg)
        .map_err(|e| format!("encode JPEG: {e}"))?;
    let jpeg_bytes = jpeg_buf.into_inner();
    use base64::{engine::general_purpose, Engine as _};
    let b64 = general_purpose::STANDARD.encode(&jpeg_bytes);
    Ok(format!("data:image/jpeg;base64,{b64}"))
}

/// Desktop / iOS: raw bytes via `tauri::ipc::Response` (optimal, no JSON encoding).
/// Note: Android WebView doesn't support `InvokeBody::Raw`, so the mobile-frontend
/// uses `read_materialized_bytes_b64` instead.
#[cfg(not(target_os = "android"))]
#[tauri::command]
fn read_materialized_bytes(
    app: tauri::AppHandle,
    asset_path: String,
) -> Result<tauri::ipc::Response, String> {
    let bytes = materialize::read_materialized_file(&app, &asset_path)?;
    Ok(tauri::ipc::Response::new(bytes))
}

/// Android: base64-encoded string. ~33% size overhead but far cheaper than JSON `number[]`
/// (one byte → one JS number would be ~3-4x raw size and 4-byte ints).
/// Frontend decodes via `fetch('data:application/octet-stream;base64,...')` or `atob`.
#[tauri::command]
fn read_materialized_bytes_b64(
    app: tauri::AppHandle,
    asset_path: String,
) -> Result<String, String> {
    use base64::Engine;
    let bytes = materialize::read_materialized_file(&app, &asset_path)?;
    Ok(base64::engine::general_purpose::STANDARD.encode(&bytes))
}

/// Read raw bytes for a `content://` / `file://` uri and return base64.
///
/// The Tauri Android WebView serves pages from the secure context
/// `http://tauri.localhost`, which blocks cross-origin JS `fetch()`/XHR to the
/// `http://127.0.0.1:<port>` stream server. Formats that need raw bytes in JS
/// (aiff->wav transcode, pdf.js) therefore fetch over Tauri IPC instead of the
/// HTTP stream server. Media elements (`<audio>/<video>/<img>`) still load the
/// stream across origins fine, so those keep using `stream_url`.
#[tauri::command]
async fn read_uri_bytes_b64(app: tauri::AppHandle, uri: String) -> Result<String, String> {
    use base64::Engine;
    let bytes = uri_util::read_uri_bytes_open(&app, &uri)?;
    Ok(base64::engine::general_purpose::STANDARD.encode(&bytes))
}

/// Desktop: raw bytes over Tauri IPC (zero-copy `ArrayBuffer`).
#[cfg(not(target_os = "android"))]
#[tauri::command]
async fn read_uri_bytes(
    app: tauri::AppHandle,
    uri: String,
) -> Result<tauri::ipc::Response, String> {
    let bytes = uri_util::read_uri_bytes_open(&app, &uri)?;
    Ok(tauri::ipc::Response::new(bytes))
}

/// PPTX viewer: materialize if needed, return cache file URL (offline).
#[tauri::command]
fn ensure_pptx_asset(
    app: tauri::AppHandle,
    uri: String,
    name: Option<String>,
) -> Result<String, String> {
    let (ext, display_name) = uri_util::uri_display_name(&app, &uri, name);
    let ext = if ext.is_empty() { "pptx".into() } else { ext };
    let doc = materialize::open_pptx_materialized(&app, &uri, &display_name, &ext)?;
    match doc {
        viewit_core_types::Document::Pptx { asset_path, .. } => {
            if asset_path.is_empty() {
                Err("pptx materialize produced empty path".into())
            } else {
                Ok(asset_path)
            }
        }
        _ => Err("expected pptx document".into()),
    }
}

#[tauri::command]
fn probe_uri(
    app: tauri::AppHandle,
    uri: String,
    name: Option<String>,
) -> Result<Option<Document>, String> {
    uri_util::probe_before_read(&app, &uri, name)
}

#[derive(serde::Serialize)]
struct BrowseEntry {
    name: String,
    path: String,
    is_dir: bool,
    size: u64,
}

#[derive(serde::Serialize)]
struct BrowseListing {
    path: String,
    parent: Option<String>,
    entries: Vec<BrowseEntry>,
    total: usize,
    has_more: bool,
}

fn read_browse_dir(path: Option<String>, offset: usize, limit: usize) -> Result<BrowseListing, String> {
    let path = match path {
        Some(p) if !p.trim().is_empty() => p
            .strip_prefix("file://")
            .unwrap_or(&p)
            .trim_end_matches('/')
            .to_string(),
        // Android starts in the user's Download directory (the demo corpus
        // home); fall back to the external storage root.
        _ => {
            for candidate in ["/sdcard/Download", "/sdcard"] {
                if std::path::Path::new(candidate).is_dir() {
                    return read_browse_dir(Some(candidate.to_string()), offset, limit);
                }
            }
            return Err("no browsable storage root found".into());
        }
    };
    let dir = std::path::PathBuf::from(&path);
    let mut entries: Vec<BrowseEntry> = Vec::new();
    let read_dir =
        std::fs::read_dir(&dir).map_err(|e| format!("cannot list {}: {e}", dir.display()))?;
    for item in read_dir.flatten() {
        let Ok(meta) = item.metadata() else { continue };
        let name = item.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        entries.push(BrowseEntry {
            name,
            path: item.path().to_string_lossy().into_owned(),
            is_dir: meta.is_dir(),
            size: meta.len(),
        });
    }
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    // Window the sorted set so huge directories ship in bounded pages.
    let total = entries.len();
    let offset = offset.min(total);
    let limit = limit.clamp(1, 1000);
    let page: Vec<BrowseEntry> = entries.into_iter().skip(offset).take(limit).collect();
    let has_more = offset + page.len() < total;
    let parent = dir.parent().map(|p| p.to_string_lossy().into_owned());
    Ok(BrowseListing {
        path: dir.to_string_lossy().into_owned(),
        parent,
        entries: page,
        total,
        has_more,
    })
}

/// Real filesystem browsing for the app's Browse mode. On Android 11+ this
/// requires the user to grant "All files access"; the frontend gates on the
/// AndroidBridge permission helpers before invoking.
#[tauri::command]
fn browse_dir(
    path: Option<String>,
    offset: Option<usize>,
    limit: Option<usize>,
) -> Result<BrowseListing, String> {
    read_browse_dir(path, offset.unwrap_or(0), limit.unwrap_or(300))
}

#[derive(serde::Serialize)]
struct FuzzyFileMatch {
    name: String,
    path: String,
    is_dir: bool,
    score: i32,
}

#[tauri::command]
fn search_files_fuzzy(query: String, root: Option<String>, limit: Option<usize>) -> Result<Vec<FuzzyFileMatch>, String> {
    use fuzzy_matcher::FuzzyMatcher;
    use fuzzy_matcher::skim::SkimMatcherV2;
    let matcher = SkimMatcherV2::default();
    let root_path = match root {
        Some(p) if !p.trim().is_empty() => p
            .strip_prefix("file://")
            .unwrap_or(&p)
            .trim_end_matches('/')
            .to_string(),
        _ => {
            for candidate in ["/sdcard/Download", "/sdcard"] {
                if std::path::Path::new(candidate).is_dir() {
                    return search_files_fuzzy(query, Some(candidate.to_string()), limit);
                }
            }
            return Err("no browsable storage root found".into());
        }
    };
    let max_results = limit.unwrap_or(50).clamp(1, 200);
    let mut results: Vec<FuzzyFileMatch> = Vec::new();
    fn walk(
        dir: &std::path::Path,
        query: &str,
        matcher: &SkimMatcherV2,
        results: &mut Vec<FuzzyFileMatch>,
        max: usize,
        depth: usize,
    ) {
        if results.len() >= max || depth > 8 { return; }
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            if results.len() >= max { return; }
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || name == "Android" { continue; }
            if let Some(score) = matcher.fuzzy_match(&name, query) {
                if score > 0 {
                    results.push(FuzzyFileMatch {
                        name: name.clone(),
                        path: entry.path().to_string_lossy().into_owned(),
                        is_dir: entry.file_type().map(|t| t.is_dir()).unwrap_or(false),
                        score: score as i32,
                    });
                }
            }
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                walk(&entry.path(), query, matcher, results, max, depth + 1);
            }
        }
    }
    walk(std::path::Path::new(&root_path), &query, &matcher, &mut results, max_results, 0);
    results.sort_by(|a, b| b.score.cmp(&a.score));
    results.truncate(max_results);
    Ok(results)
}

#[derive(serde::Serialize)]
struct ContentSearchMatch {
    path: String,
    line_number: u64,
    line_text: String,
}

fn search_office_zip_entry(
    zip_path: &std::path::Path,
    entry_pattern: &str,
    query_lower: &str,
    results: &mut Vec<ContentSearchMatch>,
    max: usize,
) {
    if results.len() >= max { return; }
    let Ok(file) = std::fs::File::open(zip_path) else { return };
    let Ok(mut archive) = zip::ZipArchive::new(file) else { return };
    for i in 0..archive.len() {
        if results.len() >= max { return; }
        let Ok(mut entry) = archive.by_index(i) else { continue };
        let name = entry.name().to_string();
        if name.contains(entry_pattern) {
            use std::io::BufRead;
            let reader = std::io::BufReader::new(&mut entry);
            let mut line_num = 1;
            for line_res in reader.lines() {
                if results.len() >= max { return; }
                if let Ok(line) = line_res {
                    // Strip basic XML tags to extract readable text
                    let stripped: String = line
                        .split('<')
                        .filter_map(|part| part.split_once('>').map(|(_, text)| text))
                        .collect::<Vec<_>>()
                        .join(" ");
                    let haystack = if stripped.is_empty() { &line } else { &stripped };
                    if haystack.to_lowercase().contains(query_lower) {
                        results.push(ContentSearchMatch {
                            path: zip_path.to_string_lossy().into_owned(),
                            line_number: line_num,
                            line_text: haystack.trim().chars().take(80).collect(),
                        });
                    }
                    line_num += 1;
                }
            }
        }
    }
}

fn is_searchable_text_file(path: &std::path::Path) -> bool {
    const BINARY_EXTENSIONS: &[&str] = &[
        "mp3", "wav", "flac", "ogg", "oga", "m4a", "aac", "opus", "aiff", "aif", "wma", "mid", "midi",
        "mp4", "mkv", "mov", "avi", "webm", "m4v", "wmv", "flv", "ts", "3gp",
        "png", "jpg", "jpeg", "gif", "webp", "bmp", "ico", "heic", "heif", "avif", "tif", "tiff",
        "zip", "7z", "rar", "tar", "gz", "bz2", "xz", "zst", "lz4", "lzma", "cab", "iso", "jar", "apk",
        "exe", "dll", "so", "dylib", "bin", "dat", "class", "pyc", "wasm", "pdf",
        "docx", "docm", "dotx", "pptx", "pptm", "potx", "xlsx", "xlsm", "xltx", "odt", "ods", "odp", "epub"
    ];

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());

    if let Some(ext) = ext {
        if BINARY_EXTENSIONS.contains(&ext.as_str()) {
            return false;
        }
    }
    true
}

#[tauri::command]
fn search_files_content(
    query: String,
    root: Option<String>,
    limit: Option<usize>,
    is_regex: Option<bool>,
) -> Result<Vec<ContentSearchMatch>, String> {
    use grep_regex::RegexMatcher;
    use grep_searcher::SearcherBuilder;
    use grep_searcher::sinks::UTF8;
    let root_path = match root {
        Some(p) if !p.trim().is_empty() => p
            .strip_prefix("file://")
            .unwrap_or(&p)
            .trim_end_matches('/')
            .to_string(),
        _ => {
            for candidate in ["/sdcard/Download", "/sdcard"] {
                if std::path::Path::new(candidate).is_dir() {
                    return search_files_content(query, Some(candidate.to_string()), limit, is_regex);
                }
            }
            return Err("no browsable storage root found".into());
        }
    };
    let max_results = limit.unwrap_or(50).clamp(1, 500);
    let pattern = if is_regex.unwrap_or(false) {
        query.clone()
    } else {
        regex::escape(&query)
    };
    let matcher = RegexMatcher::new(&pattern).map_err(|e| format!("invalid search pattern: {}", e))?;
    let mut results: Vec<ContentSearchMatch> = Vec::new();
    let mut searcher = SearcherBuilder::new()
        .line_number(true)
        .build();
    fn walk_content(
        dir: &std::path::Path,
        query: &str,
        matcher: &grep_regex::RegexMatcher,
        searcher: &mut grep_searcher::Searcher,
        results: &mut Vec<ContentSearchMatch>,
        max: usize,
        depth: usize,
    ) {
        if results.len() >= max || depth > 5 { return; }
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            if results.len() >= max { return; }
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || name == "Android" || name == "data" || name == "obb" { continue; }
            let ft = entry.file_type();
            if ft.as_ref().map(|t| t.is_dir()).unwrap_or(false) {
                walk_content(&entry.path(), query, matcher, searcher, results, max, depth + 1);
            } else if ft.as_ref().map(|t| t.is_file()).unwrap_or(false) {
                let path = entry.path();
                let ext = path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).unwrap_or_default();
                
                if matches!(ext.as_str(), "docx" | "docm" | "dotx" | "pptx" | "pptm" | "potx" | "xlsx" | "xlsm" | "xltx" | "odt" | "ods" | "odp" | "epub") {
                    let query_lower = query.to_lowercase();
                    match ext.as_str() {
                        "docx" | "docm" | "dotx" => search_office_zip_entry(&path, "word/document.xml", &query_lower, results, max),
                        "pptx" | "pptm" | "potx" => search_office_zip_entry(&path, "ppt/slides/slide", &query_lower, results, max),
                        "xlsx" | "xlsm" | "xltx" => search_office_zip_entry(&path, "xl/sharedStrings.xml", &query_lower, results, max),
                        "odt" | "ods" | "odp" => search_office_zip_entry(&path, "content.xml", &query_lower, results, max),
                        "epub" => search_office_zip_entry(&path, ".xhtml", &query_lower, results, max),
                        _ => {}
                    }
                    continue;
                }

                if !is_searchable_text_file(&path) {
                    continue;
                }
                let meta = entry.metadata();
                if meta.as_ref().map(|m| m.len() > 500_000).unwrap_or(true) { continue; }
                let sink = UTF8(|line_num, line| {
                    if results.len() < max {
                        results.push(ContentSearchMatch {
                            path: path.to_string_lossy().into_owned(),
                            line_number: line_num,
                            line_text: line.to_string().trim_end().to_string(),
                        });
                    }
                    Ok(results.len() < max)
                });
                let _ = searcher.search_path(matcher, &path, sink);
            }
        }
    }
    walk_content(std::path::Path::new(&root_path), &query, &matcher, &mut searcher, &mut results, max_results, 0);
    Ok(results)
}

/// Picker path: raw `Vec<u8>` from Tauri IPC (not JSON `number[]`).
#[tauri::command]
fn open_bytes(bytes: Vec<u8>, name: String) -> Result<Document, String> {
    uri_util::open_from_bytes(bytes, name)
}

#[tauri::command]
fn open_bytes_b64(b64: String, name: String) -> Result<Document, String> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .map_err(|e| format!("base64 decode: {}", e))?;
    uri_util::open_from_bytes(bytes, name)
}

/// Register a materialized cache file with the stream protocol and return
/// a `http://127.0.0.1:{port}/{id}` URL. The WebView's `<video>` / `<audio>`
/// can seek natively via HTTP Range requests — no base64 IPC needed.
#[tauri::command]
fn media_stream_url(
    app: tauri::AppHandle,
    asset_path: String,
    ext: String,
) -> Result<String, String> {
    let file_path = if asset_path.starts_with("file://") {
        url::Url::parse(&asset_path)
            .map_err(|e| e.to_string())?
            .to_file_path()
            .map_err(|_| "bad file url".to_string())?
    } else {
        std::path::PathBuf::from(&asset_path)
    };
    if !file_path.exists() {
        return Err(format!("file not found: {}", file_path.display()));
    }
    let slots = app.state::<stream_protocol::StreamSlots>();
    let id = stream_protocol::insert_cached(&slots, file_path, ext);
    let port = app
        .state::<HttpPort>()
        .0
        .load(std::sync::atomic::Ordering::Relaxed);
    if port > 0 {
        Ok(format!("http://127.0.0.1:{}/{}", port, id))
    } else {
        Err("HTTP server not running".into())
    }
}

/// Register any URI (content:// or file://) with the new stream server
/// and return a `http://127.0.0.1:{port}/{id}` URL.
#[tauri::command]
fn register_stream_uri(app: tauri::AppHandle, uri: String) -> Result<String, String> {
    let registry = app.state::<stream_server::StreamRegistry>();
    let id = stream_server::register_uri(&registry, uri);
    let port = app
        .state::<HttpPort>()
        .0
        .load(std::sync::atomic::Ordering::Relaxed);
    if port > 0 {
        Ok(format!("http://127.0.0.1:{}/{}", port, id))
    } else {
        Err("HTTP server not running".into())
    }
}

#[cfg(feature = "fmt-pdf")]
#[tauri::command]
async fn pdf_page(app: tauri::AppHandle, uri: String, index: usize) -> Result<String, String> {
    uri_util::pdf_page_from_uri(&app, uri, index)
}

#[tauri::command]
async fn text_page(
    app: tauri::AppHandle,
    uri: String,
    offset: usize,
    len: usize,
) -> Result<String, String> {
    let (bytes, _, _) = uri_util::read_uri_meta(&app, &uri, None)?;
    let text = String::from_utf8_lossy(&bytes);
    let end = (offset + len).min(text.len());
    let start = offset.min(text.len());
    Ok(text[start..end].to_string())
}

#[tauri::command]
async fn csv_page(
    app: tauri::AppHandle,
    uri: String,
    skip: usize,
    take: usize,
) -> Result<Vec<Vec<String>>, String> {
    let (bytes, ext, _) = uri_util::read_uri_meta(&app, &uri, None)?;
    let text = String::from_utf8_lossy(&bytes);
    let sep = if ext == "tsv" { b'\t' } else { b',' };
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(sep)
        .has_headers(true)
        .flexible(true)
        .from_reader(text.as_bytes());
    let mut rows: Vec<Vec<String>> = Vec::with_capacity(take);
    let mut idx = 0;
    for result in rdr.records() {
        if idx < skip {
            idx += 1;
            continue;
        }
        if rows.len() >= take {
            break;
        }
        match result {
            Ok(r) => rows.push(r.iter().map(|c| c.to_string()).collect()),
            Err(_) => break,
        }
        idx += 1;
    }
    Ok(rows)
}

#[tauri::command]
async fn archive_extract(
    app: tauri::AppHandle,
    uri: String,
    entry_name: String,
) -> Result<Vec<u8>, String> {
    let (bytes, ext, _) = uri_util::read_uri_meta(&app, &uri, None)?;
    let cursor = std::io::Cursor::new(bytes);
    match ext.as_str() {
        "zip" => {
            let mut archive = zip::ZipArchive::new(cursor).map_err(|e| e.to_string())?;
            let mut f = archive.by_name(&entry_name).map_err(|e| e.to_string())?;
            let mut out = Vec::with_capacity(f.size() as usize);
            f.read_to_end(&mut out).map_err(|e| e.to_string())?;
            Ok(out)
        }
        "tar" => {
            let mut archive = tar::Archive::new(cursor);
            for entry in archive.entries().map_err(|e| e.to_string())? {
                let Ok(mut entry) = entry else {
                    continue;
                };
                if entry
                    .path()
                    .map(|p| p.to_string_lossy() == entry_name)
                    .unwrap_or(false)
                {
                    let mut out = Vec::new();
                    entry.read_to_end(&mut out).map_err(|e| e.to_string())?;
                    return Ok(out);
                }
            }
            Err(format!("entry not found: {}", entry_name))
        }
        "gz" => {
            // Try tar.gz first: decompress and look for entry
            use flate2::read::GzDecoder;
            use std::io::Read;
            let mut decoder = GzDecoder::new(cursor.clone());
            let mut header = [0u8; 512];
            let n = decoder.read(&mut header).unwrap_or(0);
            if n >= 268 && header.starts_with(b"ustar") {
                // It's a tar.gz — find the entry
                let mut archive = tar::Archive::new(GzDecoder::new(cursor));
                for entry in archive.entries().map_err(|e| e.to_string())? {
                    let Ok(mut entry) = entry else {
                        continue;
                    };
                    if entry
                        .path()
                        .map(|p| p.to_string_lossy() == entry_name)
                        .unwrap_or(false)
                    {
                        let mut out = Vec::new();
                        entry.read_to_end(&mut out).map_err(|e| e.to_string())?;
                        return Ok(out);
                    }
                }
                Err(format!("entry not found: {}", entry_name))
            } else {
                // Plain gz — decompress and return raw bytes
                let mut decoder = GzDecoder::new(cursor);
                let mut out = Vec::new();
                decoder.read_to_end(&mut out).map_err(|e| e.to_string())?;
                Ok(out)
            }
        }
        _ => Err(format!("extraction not supported for .{} archive", ext)),
    }
}

#[tauri::command]
async fn epub_chapter(app: tauri::AppHandle, uri: String, index: usize) -> Result<String, String> {
    let (bytes, _, _) = uri_util::read_uri_meta(&app, &uri, None)?;
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|e| e.to_string())?;
    let mut container = String::new();
    if let Ok(mut f) = archive.by_name("META-INF/container.xml") {
        f.read_to_string(&mut container).ok();
    }
    let opf_path = extract_opf_path(&container).ok_or_else(|| "no OPF path".to_string())?;
    let mut opf = String::new();
    if let Ok(mut f) = archive.by_name(&opf_path) {
        f.read_to_string(&mut opf).ok();
    }
    let spine = extract_spine_hrefs(&opf, &opf_path);
    if index >= spine.len() {
        return Err(format!(
            "chapter index {} out of range (spine len {})",
            index,
            spine.len()
        ));
    }
    let chapter_path = &spine[index];
    let mut chapter = String::new();
    if let Ok(mut f) = archive.by_name(chapter_path) {
        f.read_to_string(&mut chapter).ok();
    }
    Ok(inline_epub_images(&mut archive, chapter_path, &chapter))
}

fn inline_epub_images<R>(archive: &mut zip::ZipArchive<R>, chapter_path: &str, html: &str) -> String
where
    R: Read + std::io::Seek,
{
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some((attr_start, attr_len, quote, value_start)) = find_image_attr(rest) {
        out.push_str(&rest[..attr_start + attr_len]);
        let Some(value_end_rel) = rest[value_start..].find(quote) else {
            out.push_str(&rest[attr_start + attr_len..]);
            return out;
        };
        let value_end = value_start + value_end_rel;
        let src = &rest[value_start..value_end];
        out.push(quote);
        out.push_str(
            &image_data_url(archive, chapter_path, src).unwrap_or_else(|| src.to_string()),
        );
        out.push(quote);
        rest = &rest[value_end + quote.len_utf8()..];
    }
    out.push_str(rest);
    out
}

fn find_image_attr(text: &str) -> Option<(usize, usize, char, usize)> {
    let lower = text.to_ascii_lowercase();
    ["src=", "href=", "xlink:href="]
        .iter()
        .filter_map(|attr| lower.find(attr).map(|idx| (idx, attr.len())))
        .min_by_key(|(idx, _)| *idx)
        .and_then(|(idx, len)| {
            let quote_idx = idx + len;
            let quote = *text.as_bytes().get(quote_idx)? as char;
            if quote != '\'' && quote != '"' {
                return None;
            }
            Some((idx, len, quote, quote_idx + 1))
        })
}

fn image_data_url<R>(
    archive: &mut zip::ZipArchive<R>,
    chapter_path: &str,
    src: &str,
) -> Option<String>
where
    R: Read + std::io::Seek,
{
    if src.starts_with("data:") || src.starts_with("http:") || src.starts_with("https:") {
        return None;
    }
    let src = src.split('#').next().unwrap_or(src);
    let path = normalize_relative_path(chapter_path, src);
    let mime = image_mime(&path)?;
    let mut file = archive.by_name(&path).ok()?;
    let mut bytes = Vec::with_capacity(file.size() as usize);
    file.read_to_end(&mut bytes).ok()?;
    Some(format!(
        "data:{};base64,{}",
        mime,
        general_purpose::STANDARD.encode(bytes)
    ))
}

fn normalize_relative_path(base_file: &str, rel: &str) -> String {
    let mut parts: Vec<&str> = base_file.split('/').collect();
    parts.pop();
    for part in rel.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            _ => parts.push(part),
        }
    }
    parts.join("/")
}

fn image_mime(path: &str) -> Option<&'static str> {
    match path.rsplit('.').next()?.to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => Some("image/jpeg"),
        "png" => Some("image/png"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "svg" => Some("image/svg+xml"),
        _ => None,
    }
}

fn extract_opf_path(container_xml: &str) -> Option<String> {
    let key = "full-path=\"";
    let i = container_xml.find(key)? + key.len();
    let j = container_xml[i..].find('"')? + i;
    Some(container_xml[i..j].to_string())
}

fn extract_spine_hrefs(opf: &str, opf_path: &str) -> Vec<String> {
    use std::collections::HashMap;
    let mut manifest: HashMap<String, String> = HashMap::new();
    let mut in_manifest = false;
    let mut in_spine = false;
    let mut spine_idrefs: Vec<String> = Vec::new();
    for token in opf.split('<') {
        let lower = token.to_lowercase();
        if lower.starts_with("manifest") {
            in_manifest = true;
            continue;
        }
        if lower.starts_with("/manifest") {
            in_manifest = false;
            continue;
        }
        if lower.starts_with("spine") {
            in_spine = true;
            continue;
        }
        if lower.starts_with("/spine") {
            in_spine = false;
            continue;
        }
        if in_manifest && lower.starts_with("item ") {
            let mut id = None;
            let mut href = None;
            if let Some(i) = token.find("id=\"") {
                let s = i + 4;
                if let Some(e) = token[s..].find('"') {
                    id = Some(token[s..s + e].to_string());
                }
            }
            if let Some(i) = token.find("href=\"") {
                let s = i + 6;
                if let Some(e) = token[s..].find('"') {
                    href = Some(token[s..s + e].to_string());
                }
            }
            if let (Some(i), Some(h)) = (id, href) {
                manifest.insert(i, resolve_relative(h, opf_path));
            }
        }
        if in_spine && lower.starts_with("itemref ") {
            if let Some(i) = token.find("idref=\"") {
                let s = i + 7;
                if let Some(e) = token[s..].find('"') {
                    spine_idrefs.push(token[s..s + e].to_string());
                }
            }
        }
    }
    spine_idrefs
        .iter()
        .filter_map(|id| manifest.get(id).cloned())
        .collect()
}

fn resolve_relative(href: String, base: &str) -> String {
    normalize_relative_path(base, &href)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(target_os = "android")]
    {
        // FIRST line — direct native log write before any other code so we can
        // confirm `run()` is being entered even if Tauri panics elsewhere.
        android_log_write_info("viewit", "[viewit] run() FIRST LINE");
        android_logger::init_once(
            android_logger::Config::default()
                .with_max_level(log::LevelFilter::Info)
                .with_tag("viewit"),
        );
        log::set_max_level(log::LevelFilter::Info);
        android_log_write_info("viewit", "[viewit] android_logger wired");
        log::info!("[viewit] run() entered — log crate is live");
    }
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .manage(OpenedUrls(Mutex::new(vec![])))
        .manage(HttpPort::default())
        .manage(PendingDisplayNames::default())
        .manage(stream_protocol::StreamSlots::default())
        .manage(stream_server::StreamRegistry::default());
    #[cfg(target_os = "android")]
    let builder = builder.manage(android_pending::PendingUrisWithMime::new());
    builder
        .register_uri_scheme_protocol("viewit-stream", |ctx, request| {
            let app = ctx.app_handle();
            let slots = app.state::<stream_protocol::StreamSlots>();
            stream_protocol::handle_stream_request_http(app, slots.inner(), request)
        })
        .invoke_handler(tauri::generate_handler![
            opened_urls,
            read_materialized_bytes_b64,
            read_uri_bytes_b64,
            #[cfg(not(target_os = "android"))]
            read_uri_bytes,
            #[cfg(not(target_os = "android"))]
            read_materialized_bytes,
            ensure_pptx_asset,
            probe_uri,
            open_uri,
            open_bytes,
            open_bytes_b64,
            text_page,
            csv_page,
            media_stream_url,
            register_stream_uri,
            epub_chapter,
            archive_extract,
            browse_dir,
            search_files_fuzzy,
            search_files_content,
            decode_heic_to_data_url,
            #[cfg(feature = "fmt-pdf")]
            pdf_page
        ])
        .setup(|app| {
            #[cfg(target_os = "android")]
            {
                let _ = materialize::prune_viewit_cache(app.handle());
                let entries = android_pending::drain_into_opened_urls(app.handle());
                // Store display names for later lookup
                if !entries.is_empty() {
                    let state = app.state::<PendingDisplayNames>();
                    let mut names_lock = state.0.lock().unwrap();
                    for (uri, name) in entries {
                        names_lock.insert(uri, name);
                    }
                }
                // Start the new unified stream server
                match stream_server::start(app.handle().clone()) {
                    Ok(port) => {
                        app.state::<HttpPort>()
                            .0
                            .store(port, std::sync::atomic::Ordering::Relaxed);
                        android_log_write_info(
                            "viewit",
                            &format!("[viewit] HTTP stream server on 127.0.0.1:{}", port),
                        );
                    }
                    Err(e) => {
                        android_log_write_info(
                            "viewit",
                            &format!("[viewit] HTTP stream server FAILED: {}", e),
                        );
                    }
                }
            }
            #[cfg(not(target_os = "android"))]
            let _ = app;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building viewit-mobile Tauri application")
        .run(|app, event| {
            #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]
            if let tauri::RunEvent::Opened { ref urls } = event {
                use tauri::Emitter;
                app.state::<OpenedUrls>()
                    .0
                    .lock()
                    .unwrap()
                    .extend(urls.iter().cloned());
                let _ = app.emit("opened", urls.clone());
            }
            let _ = (app, event);
        });
}

/// Direct FFI to `__android_log_write` — bypasses `log` crate entirely.
/// Used so we can confirm native logging works even if `log::set_logger`
/// was shadowed by another crate during init.
#[cfg(target_os = "android")]
#[inline(never)]
fn android_log_write_info(tag: &str, msg: &str) {
    use std::ffi::CString;
    unsafe {
        let tag_c = CString::new(tag).unwrap_or_default();
        let msg_c = CString::new(msg).unwrap_or_default();
        extern "C" {
            fn __android_log_write(prio: i32, tag: *const i8, text: *const i8) -> i32;
        }
        const ANDROID_LOG_INFO: i32 = 4;
        let r = __android_log_write(
            ANDROID_LOG_INFO,
            tag_c.as_ptr() as *const i8,
            msg_c.as_ptr() as *const i8,
        );
        // sink the return value so LTO doesn't drop the call.
        std::hint::black_box(r);
    }
}
