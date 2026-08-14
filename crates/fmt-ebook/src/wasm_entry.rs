//! WASM entry point for viewit-fmt-ebook.

use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}

/// Parse ebook bytes to a JSON Document string.
#[wasm_bindgen]
pub fn render(bytes: &[u8], ext: &str) -> Result<String, JsError> {
    crate::render(bytes, ext).map_err(|e| JsError::new(&e.to_string()))
}

#[wasm_bindgen]
pub fn supported_formats() -> Vec<String> {
    vec!["epub".into(), "mobi".into(), "azw3".into()]
}

#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").into()
}

#[wasm_bindgen]
pub fn name() -> String {
    "ebook-universal".into()
}
