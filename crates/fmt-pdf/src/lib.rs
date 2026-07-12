//! Phase 2.2 crate — PDF. Per ADR 0002 we bundle `libpdfium.so` (arm64).
//! Phase 1: returns Placeholder.

use viewit_core_types::{Document, Format, Error};

#[allow(dead_code)]
pub fn parse(bytes: &[u8], format: Format, name: &str) -> Result<Document, Error> {
    Ok(Document::Placeholder { format, name: name.to_string(), byte_len: bytes.len() })
}
