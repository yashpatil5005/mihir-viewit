//! Phase 4 crate — Apple iWork (.pages/.numbers/.key) per ADR 0003.

use viewit_core_types::{Document, Format, Error};

#[allow(dead_code)]
pub fn parse(bytes: &[u8], format: Format, name: &str) -> Result<Document, Error> {
    Ok(Document::Placeholder { format, name: name.to_string(), byte_len: bytes.len() })
}
