//! Android MainActivity writes share/view URIs to this file before the WebView is ready.

use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use tauri::{AppHandle, Emitter, Manager, Url};

use crate::OpenedUrls;

pub fn pending_file_path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_data_dir().ok().map(|p| p.join("viewit_pending_opens.txt"))
}

pub fn drain_into_opened_urls(app: &AppHandle) -> Vec<String> {
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
        if let Ok(url) = Url::parse(line) {
            app.state::<OpenedUrls>()
                .0
                .lock()
                .unwrap()
                .push(url);
            added.push(line.to_string());
        }
    }
    let _ = std::fs::remove_file(&path);
    if !added.is_empty() {
        let _ = app.emit("opened", added.clone());
    }
    added
}