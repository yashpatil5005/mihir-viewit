//! Phase 2.1 crate — image pass-through. Per plan §5: native webview decoders.
//! Rust crate stays a tiny stub on purpose; the webview's <img> does the work.
//! Phase 1: Placeholder for now; Phase 2.1 wires `convertFileSrc` on the frontend.

use viewit_core_types::{Document, Format, Error};

#[allow(dead_code)]
pub fn parse(bytes: &[u8], format: Format, name: &str) -> Result<Document, Error> {
    Ok(Document::Image {
        format,
        byte_len: bytes.len(),
        name: name.to_string(),
        asset_path: String::new(),
    })
}
