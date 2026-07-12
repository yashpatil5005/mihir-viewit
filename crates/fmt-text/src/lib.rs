//! Phase 1 plain-text parsing. Phase 2 will add markdown/json/csv/code paths.
//!
//! Phase 1.10 (size-budget baseline) is treated as lossy UTF-8 — the encoding
//! detection via `chardetng` lands later in 1.10 work. For now BOM is stripped
//! and everything else is lossily decoded.
//!
//! Returned variant is always `Document::Text`. The frontend routes by the
//! originally-sniffed `Format` if it wants different rendering (e.g. Markdown
//! runs through `pulldown-cmark` later in Phase 2.4).

use viewit_core_types::{Document, Format, Error};

pub fn parse_text(bytes: &[u8], _format: Format, _name: &str) -> Result<Document, Error> {
    // Best-effort: strip UTF-8 BOM if present.
    let stripped: &[u8] = bytes
        .strip_prefix(b"\xEF\xBB\xBF")
        .unwrap_or(bytes);

    // Lossy UTF-8 decoder. Phase 1.10 swaps to chardetng + encoding_rs.
    let content = String::from_utf8_lossy(stripped).into_owned();

    Ok(Document::Text {
        content,
        encoding: "utf-8".to_string(),
        byte_len: stripped.len(),
    })
}
