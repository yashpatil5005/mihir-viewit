//! ViewIt mobile Tauri backend. Mirrors desktop commands; Android uses
//! `tauri-plugin-fs` for `content://` URIs (share / file picker).

use std::io::Read;
use std::str::FromStr;
use std::sync::Mutex;

use tauri::{Manager, Url};
use tauri_plugin_fs::{FilePath, FsExt};
use viewit_core::{open, Document};

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

#[tauri::command]
async fn open_uri(
    app: tauri::AppHandle,
    uri: String,
    name: Option<String>,
) -> Result<Document, String> {
    let (bytes, ext, display_name) = read_uri_meta(&app, &uri, name)?;
    open(&bytes, &ext, &display_name).map_err(|e| e.to_string())
}

#[tauri::command]
async fn text_page(
    app: tauri::AppHandle,
    uri: String,
    offset: usize,
    len: usize,
) -> Result<String, String> {
    let (bytes, _, _) = read_uri_meta(&app, &uri, None)?;
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
    let (bytes, ext, _) = read_uri_meta(&app, &uri, None)?;
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
    let (bytes, ext, _) = read_uri_meta(&app, &uri, None)?;
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
        _ => Err(format!("extraction not supported for .{} archive", ext)),
    }
}

#[tauri::command]
async fn epub_chapter(app: tauri::AppHandle, uri: String, index: usize) -> Result<String, String> {
    let (bytes, _, _) = read_uri_meta(&app, &uri, None)?;
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
    Ok(chapter)
}

fn read_uri_meta(
    app: &tauri::AppHandle,
    uri: &str,
    name: Option<String>,
) -> Result<(Vec<u8>, String, String), String> {
    let bytes = app
        .fs()
        .read(FilePath::from_str(uri).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let (ext, display_name) = uri_display_name(uri, name);
    Ok((bytes, ext, display_name))
}

fn uri_display_name(uri: &str, name: Option<String>) -> (String, String) {
    if let Some(n) = name.filter(|s| !s.is_empty()) {
        let ext = n.rsplit('.').next().unwrap_or("").to_lowercase();
        return (ext, n);
    }
    let url = Url::parse(uri).ok();
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
    if let Some(idx) = base.rfind('/') {
        format!("{}/{}", &base[..idx], href)
    } else {
        href
    }
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
            text_page,
            csv_page,
            epub_chapter,
            archive_extract
        ])
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