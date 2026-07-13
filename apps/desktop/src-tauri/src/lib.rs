//! ViewIt desktop Tauri backend.
//!
//! Per ADR 0005: install RunEvent::Opened plumbing for file-associations.
//! Phase 1.10: invoke `viewit_core::open(bytes, ext, name)` to dispatch.
use std::sync::Mutex;
use tauri::{Manager, Url};
use viewit_core::{open, Document};

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
    let path = parse_file_uri(&uri).ok_or_else(|| format!("not a file:// URI: {}", uri))?;
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
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
    open(&bytes, &ext, &display_name).map_err(|e| e.to_string())
}

#[tauri::command]
fn open_bytes(bytes: Vec<u8>, name: String) -> Result<Document, String> {
    let ext = name
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_lowercase();
    let display = if name.is_empty() { "file".to_string() } else { name };
    open(&bytes, &ext, &display).map_err(|e| e.to_string())
}

/// Phase 2.3 — fetch a slice of a large text file as UTF-8 string.
/// `offset` and `len` are byte offsets into the already-decoded form.
#[tauri::command]
async fn text_page(uri: String, offset: usize, len: usize) -> Result<String, String> {
    let path = parse_file_uri(&uri).ok_or_else(|| format!("not a file:// URI: {}", uri))?;
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
    let path = parse_file_uri(&uri).ok_or_else(|| format!("not a file:// URI: {}", uri))?;
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&bytes);
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(text.as_bytes());
    let mut rows: Vec<Vec<String>> = Vec::with_capacity(take);
    let mut idx = 0;
    for result in rdr.records() {
        if idx < skip { idx += 1; continue; }
        if rows.len() >= take { break; }
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
    let path = parse_file_uri(&uri).ok_or_else(|| format!("not a file:// URI: {}", uri))?;
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
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
                if entry.path().map(|p| p.to_string_lossy() == entry_name).unwrap_or(false) {
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
    let path = parse_file_uri(&uri).ok_or_else(|| format!("not a file:// URI: {}", uri))?;
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
        return Err(format!("chapter index {} out of range (spine len {})", index, spine.len()));
    }
    let chapter_path = &spine[index];
    let mut chapter = String::new();
    if let Ok(mut f) = archive.by_name(chapter_path) {
        use std::io::Read;
        f.read_to_string(&mut chapter).ok();
    }
    Ok(chapter)
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
        if lower.starts_with("manifest") { in_manifest = true; continue; }
        if lower.starts_with("/manifest") { in_manifest = false; continue; }
        if lower.starts_with("spine") { in_spine = true; continue; }
        if lower.starts_with("/spine") { in_spine = false; continue; }
        if in_manifest && lower.starts_with("item ") {
            let mut id = None;
            let mut href = None;
            for _frag in token.split('"') {
                // alternate: key=", value
            }
            // simpler: capture id="..." href="..." patterns
            if let Some(i) = token.find("id=\"") {
                let s = i + 4;
                if let Some(e) = token[s..].find('"') { id = Some(token[s..s+e].to_string()); }
            }
            if let Some(i) = token.find("href=\"") {
                let s = i + 6;
                if let Some(e) = token[s..].find('"') { href = Some(token[s..s+e].to_string()); }
            }
            if let (Some(i), Some(h)) = (id, href) { manifest.insert(i, resolve_relative(h, opf_path)); }
        }
        if in_spine && lower.starts_with("itemref ") {
            if let Some(i) = token.find("idref=\"") {
                let s = i + 7;
                if let Some(e) = token[s..].find('"') {
                    spine_idrefs.push(token[s..s+e].to_string());
                }
            }
        }
    }
    spine_idrefs.iter().filter_map(|id| manifest.get(id).cloned()).collect()
}

fn resolve_relative(href: String, base: &str) -> String {
    if let Some(idx) = base.rfind('/') { format!("{}/{}", &base[..idx], href) } else { href }
}

fn parse_file_uri(uri: &str) -> Option<std::path::PathBuf> {
    let url = tauri::Url::parse(uri).ok()?;
    if url.scheme() != "file" {
        return None;
    }
    url.to_file_path().ok()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .manage(OpenedUrls(Mutex::new(vec![])))
        .invoke_handler(tauri::generate_handler![opened_urls, open_uri, open_bytes, text_page, csv_page, epub_chapter, archive_extract])
        .build(tauri::generate_context!())
        .expect("error while building viewit-desktop Tauri application")
        .run(|app, event| {
            // macOS / iOS / Android file-association events emit RunEvent::Opened.
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
            // Other desktop platforms get to fall through; Windows/Linux
            // typically use drag-drop events separately (Phase 1.6+).
            let _ = (app, event);
        });
}
