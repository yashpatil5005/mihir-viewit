//! ViewIt mobile Tauri backend. Mirrors desktop's RunEvent::Opened plumbing
//! per ADR 0005: store cold-start URIs in a Mutex, emit "opened" runtime
//! events, expose `opened_urls` command + `open_uri` command for the frontend.
//!
//! On Android, URIs from share/intent are `content://` scheme. The Tauri
//! filesystem plugin translates `file://` and (with limited support)
//! `content://` URIs into readable File handles — verified in Phase 1.9
//! on emulator. Per plan §6, RunEvent::Opened fires on cold start here too.

use std::sync::Mutex;
use tauri::{Manager, Url};
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
async fn open_uri(uri: String, name: Option<String>) -> Result<Document, String> {
    // Phase 1.9 will surface Android content:// URI handling (Probable need:
    // a tiny Kotlin bridge plugin OR tauri-plugin-fs updated support).
    // For Phase 1.10 (desktop-style first path), this only handles file://.
    let url = tauri::Url::parse(&uri).map_err(|e| e.to_string())?;
    let bytes = match url.scheme() {
        "file" => {
            let path = url.to_file_path().map_err(|_| "invalid file:// path".to_string())?;
            std::fs::read(&path).map_err(|e| e.to_string())?
        }
        other => return Err(format!("scheme '{}' not implemented in Phase 1", other)),
    };
    let path_for_ext = url
        .to_file_path()
        .map_err(|_| "no path".to_string())?;
    let ext = path_for_ext
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    let display_name = name.unwrap_or_else(|| {
        path_for_ext
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("file")
            .to_string()
    });
    open(&bytes, &ext, &display_name).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .manage(OpenedUrls(Mutex::new(vec![])))
        .invoke_handler(tauri::generate_handler![opened_urls, open_uri])
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
