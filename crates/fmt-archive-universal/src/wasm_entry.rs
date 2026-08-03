use wasm_bindgen::prelude::*;
use crate::{parse_stream, extract_entry_stream, extract_all_stream, peek_format, ArchiveManifest, Error};
use std::io::Cursor;

#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
    #[cfg(feature = "log")]
    {
        console_log::init_with_level(log::Level::Debug).ok();
        log::info!("viewit-fmt-archive-universal WASM initialized");
    }
}

/// Parse an archive from bytes and return manifest as JSON
#[wasm_bindgen(js_name = "parseArchive")]
pub fn parse_archive(bytes: &[u8], filename: &str) -> Result<String, JsError> {
    let _format = peek_format(&mut Cursor::new(bytes)).map_err(|e| JsError::new(&e.to_string()))?;
    let manifest = parse_stream(Cursor::new(bytes), filename).map_err(|e| JsError::new(&e.to_string()))?;
    serde_json::to_string(&manifest).map_err(|e| JsError::new(&e.to_string()))
}

/// Extract a single entry from archive
#[wasm_bindgen(js_name = "extractEntry")]
pub fn extract_entry(bytes: &[u8], filename: &str, entry_name: &str) -> Result<Vec<u8>, JsError> {
    let data = extract_entry_stream(Cursor::new(bytes), filename, entry_name)
        .map_err(|e| JsError::new(&e.to_string()))?;
    Ok(data)
}

/// Extract all entries from archive
#[wasm_bindgen(js_name = "extractAll")]
pub fn extract_all(bytes: &[u8], filename: &str) -> Result<String, JsError> {
    let entries = extract_all_stream(Cursor::new(bytes), filename)
        .map_err(|e| JsError::new(&e.to_string()))?;
    serde_json::to_string(&entries).map_err(|e| JsError::new(&e.to_string()))
}

/// Get list of supported file extensions
#[wasm_bindgen(js_name = "supportedFormats")]
pub fn supported_formats() -> Vec<String> {
    vec![
        "zip".into(), "tar".into(), "tgz".into(), "gz".into(),
        "tar.gz".into(), "tar.bz2".into(), "tbz2".into(),
        "tar.xz".into(), "txz".into(), "tar.zst".into(), "tzst".into(),
        "tar.lz4".into(), "tar.lzma".into(), "tlz".into(),
        "bz2".into(), "xz".into(), "zst".into(), "lz4".into(), "lzma".into(),
        "7z".into(), "rar".into(),
    ]
}

/// Get plugin version
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").into()
}

/// Get plugin name
#[wasm_bindgen]
pub fn name() -> String {
    "archive-universal".into()
}