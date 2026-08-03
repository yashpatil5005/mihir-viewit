//! WASM entry point for viewit-fmt-office-universal.
//!
//! This module provides wasm-bindgen exports for web usage.

use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
    // Logging is optional - only available if log feature is enabled
    #[cfg(feature = "log")]
    {
        console_log::init_with_level(log::Level::Debug).ok();
        log::info!("viewit-fmt-office-universal WASM initialized");
    }
}

/// Render an office document from bytes.
/// Returns a JSON string representing the Document.
#[wasm_bindgen]
pub fn render(bytes: &[u8], ext: &str) -> Result<String, JsError> {
    crate::render(bytes, ext).map_err(|e| JsError::new(&e.to_string()))
}

/// Get list of supported file extensions.
#[wasm_bindgen]
pub fn supported_formats() -> Vec<String> {
    vec![
        "docx".into(), "docm".into(), "dotx".into(), "dotm".into(),
        "xlsx".into(), "xlsm".into(), "xlsb".into(), "xls".into(),
        "pptx".into(), "pptm".into(), "potx".into(),
        "odt".into(), "ott".into(),
        "ods".into(), "ots".into(),
        "odp".into(), "otp".into(),
        "doc".into(), "ppt".into(),
    ]
}

/// Get plugin version.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").into()
}

/// Get plugin name.
#[wasm_bindgen]
pub fn name() -> String {
    "office-universal".into()
}