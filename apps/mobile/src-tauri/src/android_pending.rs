//! Android MainActivity writes share/view URIs to this file before the WebView is ready.

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Emitter, Manager, Url};

use crate::OpenedUrls;

pub fn pending_file_path(app: &AppHandle) -> Option<PathBuf> {
    // On Android, `app_data_dir()` returns the app's data directory.
    // The Android MainActivity writes to `Context.getFilesDir()` which is
    // typically `<app_data_dir>/files/`. We check both locations for compatibility.
    app.path().app_data_dir().ok().map(|p| {
        // Try the `files/` subdirectory first (where Android writes)
        let files_dir = p.join("files");
        let direct_path = files_dir.join("viewit_pending_opens.txt");
        if direct_path.exists() {
            direct_path
        } else {
            p.join("viewit_pending_opens.txt")
        }
    })
}

/// Pending URI with optional display name and MIME type hint.
#[derive(Clone, Debug, Default)]
pub struct PendingUri {
    pub uri: String,
    pub display_name: String,
    pub mime_type: String,
}

/// Shared state for pending URIs with MIME hints, populated from the
/// onNewIntent path (warm-start case where setup() already ran).
#[derive(Clone, Default)]
pub struct PendingUrisWithMime(pub Arc<Mutex<Vec<PendingUri>>>);

impl PendingUrisWithMime {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_mime(&self, uri: &str) -> Option<String> {
        let lock = self.0.lock().unwrap();
        lock.iter().find(|p| p.uri == uri).and_then(|p| {
            if p.mime_type.is_empty() {
                None
            } else {
                Some(p.mime_type.clone())
            }
        })
    }
}

pub fn drain_into_opened_urls(app: &AppHandle) -> Vec<(String, String)> {
    let Some(path) = pending_file_path(app) else {
        return vec![];
    };
    if !path.exists() {
        return vec![];
    }
    let file = match std::fs::File::open(&path) {
        Ok(f) => f,
        Err(_) => return vec![],
    };
    let mut added = Vec::new();
    let reader = BufReader::new(file);
    for line in reader.lines() {
        let Ok(line) = line else { continue };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // Line format: "uri\tdisplay_name\tmime_type" (all fields optional after uri)
        let mut parts = line.splitn(3, '\t');
        let uri_str = parts.next().unwrap_or("");
        let display_name = parts.next().unwrap_or("");
        let mime_type = parts.next().unwrap_or("");
        if let Ok(url) = Url::parse(uri_str) {
            app.state::<OpenedUrls>().0.lock().unwrap().push(url);
            added.push((uri_str.to_string(), display_name.to_string()));
        }
        // Store MIME hint for warm-start path
        if !mime_type.is_empty() {
            if let Some(state) = app.try_state::<PendingUrisWithMime>() {
                state.0.lock().unwrap().push(PendingUri {
                    uri: uri_str.to_string(),
                    display_name: display_name.to_string(),
                    mime_type: mime_type.to_string(),
                });
            }
        }
    }
    let _ = std::fs::remove_file(&path);
    if !added.is_empty() {
        let uris: Vec<String> = added.iter().map(|(uri, _)| uri.clone()).collect();
        let _ = app.emit("opened", uris);
    }
    added
}

/// Non-destructive read of pending file — populates PendingDisplayNames and
/// PendingUrisWithMime without removing the file. Called from open_uri when
/// the app is already running (warm-start path).
pub fn read_pending_mimes(app: &AppHandle) {
    let Some(path) = pending_file_path(app) else {
        return;
    };
    if !path.exists() {
        return;
    }
    let file = match std::fs::File::open(&path) {
        Ok(f) => f,
        Err(_) => return,
    };
    let reader = BufReader::new(file);
    for line in reader.lines() {
        let Ok(line) = line else { continue };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.splitn(3, '\t');
        let uri_str = parts.next().unwrap_or("");
        let display_name = parts.next().unwrap_or("");
        let mime_type = parts.next().unwrap_or("");
        if !display_name.is_empty() {
            if let Some(state) = app.try_state::<crate::PendingDisplayNames>() {
                state
                    .0
                    .lock()
                    .unwrap()
                    .insert(uri_str.to_string(), display_name.to_string());
            }
        }
        if !mime_type.is_empty() {
            if let Some(state) = app.try_state::<PendingUrisWithMime>() {
                let mut lock = state.0.lock().unwrap();
                if !lock.iter().any(|p| p.uri == uri_str) {
                    lock.push(PendingUri {
                        uri: uri_str.to_string(),
                        display_name: display_name.to_string(),
                        mime_type: mime_type.to_string(),
                    });
                }
            }
        }
    }
    // Delete the file after reading
    let _ = std::fs::remove_file(&path);
}
