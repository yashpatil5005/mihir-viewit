//! WASM entry point for viewit-fmt-font.

use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}

/// Parse font bytes to a JSON Document string.
#[wasm_bindgen]
pub fn render(bytes: &[u8], ext: &str) -> Result<String, JsError> {
    crate::render(bytes, ext).map_err(|e| JsError::new(&e.to_string()))
}

#[wasm_bindgen]
pub fn supported_formats() -> Vec<String> {
    vec!["ttf".into(), "otf".into(), "woff".into(), "ttc".into()]
}

#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").into()
}

#[wasm_bindgen]
pub fn name() -> String {
    "font-universal".into()
}
