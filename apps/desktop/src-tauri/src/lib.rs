//! ViewIt desktop Tauri backend.
//!
//! Per ADR 0005: install RunEvent::Opened plumbing for file-associations.
//! Phase 1.10: invoke `viewit_core::open(bytes, ext, name)` to dispatch.
use std::sync::Mutex;

use base64::{engine::general_purpose, Engine as _};
use tauri::{Manager, Url};
use viewit_core::{is_stream_ext, open, open_stream, sniff, Document, Format, OPEN_BYTES_CAP};

/// Resolve a `file://` URI or a raw OS filesystem path into a `PathBuf`.
/// Desktop callers pass raw paths (picker, CLI, drag-drop); macOS file
/// associations arrive as `file://` URLs. Accept both for parity.
fn resolve_path(uri: &str) -> Option<std::path::PathBuf> {
    if let Ok(url) = tauri::Url::parse(uri) {
        if url.scheme() == "file" {
            return url.to_file_path().ok();
        }
    }
    let p = std::path::PathBuf::from(uri);
    if p.is_absolute() || p.exists() {
        Some(p)
    } else {
        None
    }
}

#[cfg(feature = "fmt-pdf")]
use viewit_fmt_pdf;

/// How much of a file we read to sniff its format before deciding whether we
/// can stream it to the WebView (images, PDF, media) instead of loading the
/// whole thing into memory.
const SNIFF_READ_CAP: usize = 256 * 1024;

/// Buffer for cold-start file URIs (the webview/JS may not be listening yet
/// when RunEvent::Opened fires during a cold-start launch).
#[derive(Default, serde::Serialize)]
struct OpenedUrls(Mutex<Vec<Url>>);

#[tauri::command]
fn opened_urls(app: tauri::AppHandle) -> Vec<String> {
    app.state::<OpenedUrls>()
        .0
        .lock()
        .unwrap()
        .iter()
        .map(|u| u.to_string())
        .collect()
}

/// Read a file at `uri` (file:// Tauri asset), parse via viewit_core, return
/// the structured Document. Phase 1.10 first path: this is the "open any
/// file and render it" loop.
#[tauri::command]
async fn open_uri(uri: String, name: Option<String>) -> Result<Document, String> {
    let path = resolve_path(&uri).ok_or_else(|| format!("not a file URI or path: {}", uri))?;
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    let display_name = name.unwrap_or_else(|| {
        path.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("file")
            .to_string()
    });
    if is_stream_ext(&ext) {
        let byte_len = std::fs::metadata(&path).map(|metadata| metadata.len() as usize).unwrap_or(0);
        let asset_path = path.to_string_lossy().to_string();
        let mut document = open_stream(&ext, &display_name).map_err(|e| e.to_string())?;
        match &mut document {
            Document::Pdf {
                byte_len: length,
                asset_path: path,
                ..
            }
            | Document::Media {
                byte_len: length,
                asset_path: path,
                ..
            } => {
                *length = byte_len;
                *path = asset_path;
            }
            _ => {}
        }
        return Ok(document);
    }
    // Sniff from a small header first: images stream to the WebView via the
    // asset protocol (`convertFileSrc`) — no full-file read, no 32 MB cap, no
    // OOM on huge images.
    let prefix = read_prefix(&path, SNIFF_READ_CAP).map_err(|e| e.to_string())?;
    let format = sniff(&prefix, &ext);
    if is_image_format(format) {
        let asset_path = path.to_string_lossy().to_string();
        let byte_len = std::fs::metadata(&path)
            .map(|m| m.len() as usize)
            .unwrap_or(prefix.len());
        return Ok(Document::Image {
            format,
            byte_len,
            name: display_name,
            asset_path,
            stream_url: None,
        });
    }
    // Small file that fully fit in the sniff buffer — skip the second read.
    if prefix.len() < SNIFF_READ_CAP {
        return open(&prefix, &ext, &display_name).map_err(|e| e.to_string());
    }
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    if bytes.len() > OPEN_BYTES_CAP {
        return Err(format!(
            "file exceeds {} MB cap",
            OPEN_BYTES_CAP / 1_048_576
        ));
    }
    open(&bytes, &ext, &display_name).map_err(|e| e.to_string())
}

/// Read at most `cap` bytes (fewer if the file is smaller).
fn read_prefix(path: &std::path::Path, cap: usize) -> std::io::Result<Vec<u8>> {
    use std::io::Read;
    let f = std::fs::File::open(path)?;
    let mut buf = Vec::with_capacity(cap.min(64 * 1024));
    let mut bounded = f.take(cap as u64);
    bounded.read_to_end(&mut buf)?;
    Ok(buf)
}

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

#[cfg(feature = "fmt-pdf")]
#[tauri::command]
async fn pdf_page(uri: String, index: usize) -> Result<String, String> {
    let path = resolve_path(&uri).ok_or_else(|| format!("not a file URI or path: {}", uri))?;
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    if bytes.len() > OPEN_BYTES_CAP {
        return Err("PDF too large".into());
    }
    pdf_page_render(&bytes, index)
}

#[cfg(feature = "fmt-pdf")]
fn pdf_page_render(bytes: &[u8], index: usize) -> Result<String, String> {
    viewit_fmt_pdf::render_page(bytes, index).map_err(|e| e.to_string())
}

#[tauri::command]
fn open_bytes(bytes: Vec<u8>, name: String) -> Result<Document, String> {
    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
    let display = if name.is_empty() {
        "file".to_string()
    } else {
        name.clone()
    };
    if is_stream_ext(&ext) && bytes.len() > OPEN_BYTES_CAP {
        return open_stream(&ext, &display).map_err(|e| e.to_string());
    }
    if let Some(doc) = viewit_core::reject_if_too_large(bytes.len()) {
        return Ok(doc);
    }
    open(&bytes, &ext, &display).map_err(|e| e.to_string())
}

/// Mirror of the mobile picker path: base64-decode bytes then open.
#[tauri::command]
fn open_bytes_b64(b64: String, name: String) -> Result<Document, String> {
    let bytes = general_purpose::STANDARD
        .decode(b64.trim())
        .map_err(|e| format!("base64 decode: {}", e))?;
    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
    let display = if name.is_empty() {
        "file".to_string()
    } else {
        name.clone()
    };
    if is_stream_ext(&ext) && bytes.len() > OPEN_BYTES_CAP {
        return open_stream(&ext, &display).map_err(|e| e.to_string());
    }
    if let Some(doc) = viewit_core::reject_if_too_large(bytes.len()) {
        return Ok(doc);
    }
    open(&bytes, &ext, &display).map_err(|e| e.to_string())
}

/// Parity guard: the frontend probes URIs before open. Desktop cost is trivial —
/// return `None` and let `open_uri` do the real work.
#[tauri::command]
fn probe_uri(_app: tauri::AppHandle, _uri: String, _name: Option<String>) -> Result<Option<Document>, String> {
    Ok(None)
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
}

fn browse_default_root() -> String {
    // Desktop starts in the user's Downloads directory, falling back to HOME.
    for candidate in ["XDG_DOWNLOAD_DIR", "HOME"] {
        if let Some(dir) = std::env::var_os(candidate) {
            let dir = std::path::PathBuf::from(dir);
            if candidate == "XDG_DOWNLOAD_DIR" || dir.is_dir() {
                return dir.to_string_lossy().into_owned();
            }
        }
    }
    ".".into()
}

fn read_browse_dir(path: Option<String>) -> Result<BrowseListing, String> {
    let path = match path {
        Some(p) if !p.trim().is_empty() => p
            .strip_prefix("file://")
            .unwrap_or(&p)
            .trim_end_matches('/')
            .to_string(),
        _ => browse_default_root(),
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
    let parent = dir.parent().map(|p| p.to_string_lossy().into_owned());
    Ok(BrowseListing {
        path: dir.to_string_lossy().into_owned(),
        parent,
        entries,
    })
}

/// Real filesystem browsing for the app's Browse mode (desktop + mobile).
#[tauri::command]
fn browse_dir(path: Option<String>) -> Result<BrowseListing, String> {
    read_browse_dir(path)
}

#[tauri::command]
fn desktop_e2e_report(payload: String) -> Result<bool, String> {
    let Some(path) = std::env::var_os("VIEWIT_DESKTOP_E2E_REPORT") else {
        return Ok(false);
    };
    std::fs::write(path, payload).map_err(|error| error.to_string())?;
    Ok(true)
}

/// Raw bytes of a materialized (`asset_path`) file for JS viewers (pdf.js, media,
/// docx-preview). Desktop returns an `ipc::Response` (zero-copy `ArrayBuffer`).
#[tauri::command]
fn read_materialized_bytes(asset_path: String) -> Result<tauri::ipc::Response, String> {
    let path = resolve_path(&asset_path)
        .ok_or_else(|| format!("not a file URI or path: {}", asset_path))?;
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    Ok(tauri::ipc::Response::new(bytes))
}

/// Raw bytes of a `file://` URI or OS path.
#[tauri::command]
async fn read_uri_bytes(uri: String) -> Result<tauri::ipc::Response, String> {
    let path = resolve_path(&uri).ok_or_else(|| format!("not a file URI or path: {}", uri))?;
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    Ok(tauri::ipc::Response::new(bytes))
}

/// Return a local filesystem path the PPTX/asset viewers can read. Desktop files
/// are already local — we only normalize a `file://` URL to a path.
#[tauri::command]
fn ensure_pptx_asset(uri: String, _name: Option<String>) -> Result<String, String> {
    resolve_path(&uri)
        .map(|p| p.to_string_lossy().to_string())
        .ok_or_else(|| format!("not a file URI or path: {}", uri))
}

/// HEIC/HEIF decode to a JPEG data URL (WebView can't render HEIC natively).
/// Mirrors the mobile implementation.
#[tauri::command]
async fn decode_heic_to_data_url(
    asset_path: String,
    uri: Option<String>,
) -> Result<String, String> {
    use base64::Engine;

    let path = if asset_path.is_empty() {
        uri.as_deref().and_then(resolve_path)
    } else {
        resolve_path(&asset_path)
    };
    let Some(path) = path else {
        return Err("no path or URI for HEIC file".into());
    };
    let data = std::fs::read(&path).map_err(|e| format!("read HEIC: {e}"))?;
    let rgb = hpvcd::decode_heic_rgb8(&data).map_err(|e| format!("decode HEIC: {e}"))?;
    let img = image::RgbImage::from_raw(rgb.width, rgb.height, rgb.pixels)
        .ok_or("invalid HEIC pixel dimensions")?;
    let mut jpeg_buf = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(img)
        .write_to(&mut jpeg_buf, image::ImageFormat::Jpeg)
        .map_err(|e| format!("encode JPEG: {e}"))?;
    let b64 = general_purpose::STANDARD.encode(jpeg_buf.into_inner());
    Ok(format!("data:image/jpeg;base64,{b64}"))
}

/// Phase 2.3 — fetch a slice of a large text file as UTF-8 string.
/// `offset` and `len` are byte offsets into the already-decoded form.
#[tauri::command]
async fn text_page(uri: String, offset: usize, len: usize) -> Result<String, String> {
    let path = resolve_path(&uri).ok_or_else(|| format!("not a file URI or path: {}", uri))?;
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    // Decode lossy then slice — text encoding detection here uses the
    // fmt-text path's heuristic; for streaming we accept the minor perf cost.
    let text = String::from_utf8_lossy(&bytes);
    let end = (offset + len).min(text.len());
    let start = offset.min(text.len());
    Ok(text[start..end].to_string())
}

/// Phase 2.6 — fetch a page of CSV rows beyond the initial `preview_rows`.
/// Returns rows as Vec<Vec<String>>. `skip` is number of rows to skip
/// (including header); `take` is page size.
#[tauri::command]
async fn csv_page(uri: String, skip: usize, take: usize) -> Result<Vec<Vec<String>>, String> {
    let path = resolve_path(&uri).ok_or_else(|| format!("not a file URI or path: {}", uri))?;
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&bytes);
    let mut rdr = csv::ReaderBuilder::new()
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

/// Phase 2.9 — extract a single entry from an archive by name.
/// Re-routes through `core::open` to render the inner format's viewer.
#[tauri::command]
async fn archive_extract(uri: String, entry_name: String) -> Result<Vec<u8>, String> {
    let path = resolve_path(&uri).ok_or_else(|| format!("not a file URI or path: {}", uri))?;
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    let cursor = std::io::Cursor::new(bytes);
    match ext.as_str() {
        "zip" => {
            let mut archive = zip::ZipArchive::new(cursor).map_err(|e| e.to_string())?;
            let mut f = archive.by_name(&entry_name).map_err(|e| e.to_string())?;
            let mut out = Vec::with_capacity(f.size() as usize);
            use std::io::Read;
            f.read_to_end(&mut out).map_err(|e| e.to_string())?;
            Ok(out)
        }
        "tar" => {
            let mut archive = tar::Archive::new(cursor);
            for entry in archive.entries().map_err(|e| e.to_string())? {
                let Ok(mut entry) = entry else { continue };
                if entry
                    .path()
                    .map(|p| p.to_string_lossy() == entry_name)
                    .unwrap_or(false)
                {
                    let mut out = Vec::new();
                    use std::io::Read;
                    entry.read_to_end(&mut out).map_err(|e| e.to_string())?;
                    return Ok(out);
                }
            }
            Err(format!("entry not found: {}", entry_name))
        }
        _ => Err(format!("extraction not supported for .{} archive", ext)),
    }
}

/// Phase 2.8 — fetch a specific EPUB chapter by spine index.
/// Returns XHTML string (frontend renders in sandbox iframe).
#[tauri::command]
async fn epub_chapter(uri: String, index: usize) -> Result<String, String> {
    let path = resolve_path(&uri).ok_or_else(|| format!("not a file URI or path: {}", uri))?;
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|e| e.to_string())?;
    let mut container = String::new();
    if let Ok(mut f) = archive.by_name("META-INF/container.xml") {
        use std::io::Read;
        f.read_to_string(&mut container).ok();
    }
    let opf_path = extract_opf_path(&container).ok_or_else(|| "no OPF path".to_string())?;
    let mut opf = String::new();
    if let Ok(mut f) = archive.by_name(&opf_path) {
        use std::io::Read;
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
        use std::io::Read;
        f.read_to_string(&mut chapter).ok();
    }
    Ok(inline_epub_images(&mut archive, chapter_path, &chapter))
}

fn inline_epub_images<R>(archive: &mut zip::ZipArchive<R>, chapter_path: &str, html: &str) -> String
where
    R: std::io::Read + std::io::Seek,
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
    R: std::io::Read + std::io::Seek,
{
    if src.starts_with("data:") || src.starts_with("http:") || src.starts_with("https:") {
        return None;
    }
    let src = src.split('#').next().unwrap_or(src);
    let path = normalize_relative_path(chapter_path, src);
    let mime = image_mime(&path)?;
    let mut file = archive.by_name(&path).ok()?;
    let mut bytes = Vec::with_capacity(file.size() as usize);
    std::io::Read::read_to_end(&mut file, &mut bytes).ok()?;
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
    // crude: find full-path="..." attribute in container.xml
    let key = "full-path=\"";
    let i = container_xml.find(key)? + key.len();
    let j = container_xml[i..].find('"')? + i;
    Some(container_xml[i..j].to_string())
}

fn extract_spine_hrefs(opf: &str, opf_path: &str) -> Vec<String> {
    // crude parser: find <item id="x" href="..."/>建立一个 manifest map,
    // then walk <itemref idref="x"/> entries in spine.
    use std::collections::HashMap;
    let mut manifest: HashMap<String, String> = HashMap::new();
    let mut in_manifest = false;
    let mut in_spine = false;
    let mut spine_idrefs: Vec<String> = Vec::new();
    let mut chars = opf.chars().peekable();
    let _ = chars.by_ref().count(); // drain into iter? Not needed; we just tokenize raw.
                                    // Simple regex-like scan with split on '<'
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
            for _frag in token.split('"') {
                // alternate: key=", value
            }
            // simpler: capture id="..." href="..." patterns
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
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .manage(OpenedUrls(Mutex::new(vec![])))
        .invoke_handler(tauri::generate_handler![
            opened_urls,
            open_uri,
            open_bytes,
            open_bytes_b64,
            probe_uri,
            read_materialized_bytes,
            read_uri_bytes,
            ensure_pptx_asset,
            decode_heic_to_data_url,
            text_page,
            csv_page,
            epub_chapter,
            archive_extract,
            pdf_page,
            browse_dir,
            desktop_e2e_report
        ])
        .setup(|app| {
            // CLI/file-manager open: on Linux/Windows a file passed as an
            // argument arrives here as a raw OS path (not RunEvent::Opened).
            let opened = app.state::<OpenedUrls>();
            for arg in std::env::args_os().skip(1) {
                let Some(s) = arg.to_str() else { continue };
                if s.starts_with('-') {
                    continue;
                }
                let path = match resolve_path(s) {
                    Some(p) => p,
                    None => continue,
                };
                let path = match std::fs::canonicalize(&path) {
                    Ok(p) => p,
                    Err(_) => path,
                };
                if let Ok(url) = Url::from_file_path(&path) {
                    opened.0.lock().unwrap().push(url);
                }
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building viewit-desktop Tauri application")
        .run(|app, event| {
            // macOS file-association events emit RunEvent::Opened (requires the
            // tauri `unstable` feature). Drag-and-drop is handled uniformly in
            // the webview (Viewer.svelte) so web and desktop share one path.
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
