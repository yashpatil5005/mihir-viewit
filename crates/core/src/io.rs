//! Shared open guards (used by Tauri hosts).

use crate::{Document, Format, Suggestion, OPEN_BYTES_CAP};

pub fn reject_if_too_large(bytes_len: usize) -> Option<Document> {
    if bytes_len > OPEN_BYTES_CAP {
        Some(Document::Unsupported {
            format: Format::Unsupported,
            reason: format!(
                "File is {:.1} MB — max {} MB loaded at once. Use Share → another app for huge files.",
                bytes_len as f64 / 1_048_576.0,
                OPEN_BYTES_CAP / 1_048_576
            ),
            suggestion: Suggestion::OpenWithExternal,
        })
    } else {
        None
    }
}
