//! ViewIt mobile Tauri backend. Mirrors desktop commands; Android uses
//! `tauri-plugin-fs` for `content://` URIs (share / file picker).

#[cfg(target_os = "android")]
mod android_pending;

mod materialize;
mod stream_protocol;
mod uri_util;

use std::io::Read;
use std::sync::Mutex;

use tauri::{Manager, Url};
use viewit_core::Document;

#[derive(Default, serde::Serialize)]
struct OpenedUrls(Mutex<Vec<Url>>);

#[tauri::command]
fn opened_urls(app: tauri::AppHandle) -> Vec<String> {
    #[cfg(target_os = "android")]
    android_pending::drain_into_opened_urls(&app);
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
    uri_util::open_from_uri(&app, uri, name)
}

/// Desktop / iOS: raw bytes via `tauri::ipc::Response` (optimal, no JSON encoding).
/// Note: Android WebView doesn't support `InvokeBody::Raw`, so the mobile-frontend
/// uses `read_materialized_bytes_b64` instead.
#[cfg(not(target_os = "android"))]
#[tauri::command]
fn read_materialized_bytes(app: tauri::AppHandle, asset_path: String) -> Result<tauri::ipc::Response, String> {
    let bytes = materialize::read_materialized_file(&app, &asset_path)?;
    Ok(tauri::ipc::Response::new(bytes))
}

/// Android: base64-encoded string. ~33% size overhead but far cheaper than JSON `number[]`
/// (one byte → one JS number would be ~3-4x raw size and 4-byte ints).
/// Frontend decodes via `fetch('data:application/octet-stream;base64,...')` or `atob`.
#[tauri::command]
fn read_materialized_bytes_b64(app: tauri::AppHandle, asset_path: String) -> Result<String, String> {
    use base64::Engine;
    let bytes = materialize::read_materialized_file(&app, &asset_path)?;
    Ok(base64::engine::general_purpose::STANDARD.encode(&bytes))
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
    Ok(chapter)
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
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .manage(OpenedUrls(Mutex::new(vec![])))
        .manage(stream_protocol::StreamSlots::default())
        .register_uri_scheme_protocol("viewit-stream", |ctx, request| {
            let app = ctx.app_handle();
            let slots = app.state::<stream_protocol::StreamSlots>();
            stream_protocol::handle_stream_request_http(app, slots.inner(), request)
        })
        .invoke_handler(tauri::generate_handler![
            opened_urls,
            read_materialized_bytes_b64,
            #[cfg(not(target_os = "android"))]
            read_materialized_bytes,
            ensure_pptx_asset,
            probe_uri,
            open_uri,
            open_bytes,
            open_bytes_b64,
            text_page,
            csv_page,
            epub_chapter,
            archive_extract,
            #[cfg(feature = "fmt-pdf")]
            pdf_page
        ])
        .setup(|app| {
            #[cfg(target_os = "android")]
            {
                let _ = materialize::prune_viewit_cache(app.handle());
                android_pending::drain_into_opened_urls(app.handle());
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
            fn __android_log_write(prio: i32, tag: *const u8, text: *const u8) -> i32;
        }
        const ANDROID_LOG_INFO: i32 = 4;
        let r = __android_log_write(ANDROID_LOG_INFO, tag_c.as_ptr(), msg_c.as_ptr());
        // sink the return value so LTO doesn't drop the call.
        std::hint::black_box(r);
    }
}