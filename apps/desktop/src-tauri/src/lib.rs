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
        .manage(OpenedUrls(Mutex::new(vec![])))
        .invoke_handler(tauri::generate_handler![opened_urls, open_uri])
        .build(tauri::generate_context!())
        .expect("error while building viewit-desktop Tauri application")
        .run(|app, event| {
            // macOS / iOS / Android file-association events emit RunEvent::Opened.
            #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]
            if let tauri::RunEvent::Opened { urls } = event {
                use tauri::Emitter;
                app.state::<OpenedUrls>()
                    .0
                    .lock()
                    .unwrap()
                    .extend(urls.clone());
                let _ = app.emit("opened", urls);
            }
            // Other desktop platforms get to fall through; Windows/Linux
            // typically use drag-drop events separately (Phase 1.6+).
            let _ = (app, event);
        });
}
