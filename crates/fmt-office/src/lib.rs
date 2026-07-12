//! Phase 3 crate — initializes Phase 3.1–3.7 (DOCX/XLSX/PPTX + legacy binary + encrypted).
//! Present in Phase 1 as a feature-only stub returning Placeholder.

use viewit_core_types::{Document, Format, Error};

#[allow(dead_code)]
pub fn parse(bytes: &[u8], format: Format, name: &str) -> Result<Document, Error> {
    Ok(Document::Placeholder { format, name: name.to_string(), byte_len: bytes.len() })
}
