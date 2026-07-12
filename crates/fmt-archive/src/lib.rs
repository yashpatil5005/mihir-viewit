//! Phase 2.9 crate — archive listing/extract (zip / tar.gz / 7z).
//! RAR5 is intentionally omitted (ADR 0004) — handled in `viewit_core::dispatch`.

use viewit_core_types::{Document, Format, Error};

#[allow(dead_code)]
pub fn parse(bytes: &[u8], format: Format, name: &str) -> Result<Document, Error> {
    Ok(Document::Placeholder { format, name: name.to_string(), byte_len: bytes.len() })
}
