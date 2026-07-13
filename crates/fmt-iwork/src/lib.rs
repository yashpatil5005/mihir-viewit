//! Phase 4.1 crate — Apple iWork (.pages/.numbers/.key) per ADR 0003.
//!
//! iWork files are zips. Structure:
//! - `Index/preview.jpg` — cover preview (image, skipped)
//! - `Index/*.iwa` — Snappy-compressed protobuf (nested Layer archives)
//! - `metadata.json` / `document.yaml` — sometimes present
//!
//! Per ADR 0003: text+structure only, honest "partial preview" banner.
//! Per ADR 0006: `litchi` (iWork protobuf schemas) deferred to 0.0.1
//! placeholder — not production. So we cannot decode `.iwa` properly yet.
//!
//! MVP approach: scan zip entries for any UTF-8 text content
//! (e.g. metadata JSON, YAML, embedded .txt) and concatenate. Mark as
//! partial preview so the frontend can surface an honest banner.
//!
//! → skipped: .iwa Snappy+protobuf decode, add when litchi crate matures (ADR 0006)

use std::io::{Cursor, Read};
use viewit_core_types::{Document, Error, Format};
use zip::ZipArchive;

pub fn parse(bytes: &[u8], format: Format, _name: &str) -> Result<Document, Error> {
    let cursor = Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor).map_err(|e| Error::Parse(format!("zip: {}", e)))?;

    let mut text_parts: Vec<String> = Vec::new();
    let mut preview_jpg: Option<Vec<u8>> = None;

    for i in 0..archive.len() {
        let Ok(mut entry) = archive.by_index(i) else {
            continue;
        };
        let entry_name = entry.name().to_string();
        let mut buf = Vec::new();
        if entry.read_to_end(&mut buf).is_err() {
            continue;
        }

        if entry_name.ends_with(".iwa") {
            // Snappy-compressed protobuf — needs litchi (ADR 0006 deferred).
            continue;
        }
        if entry_name.ends_with("preview.jpg") || entry_name.ends_with("preview.jpg-medium") {
            if preview_jpg.is_none() {
                preview_jpg = Some(buf);
            }
            continue;
        }
        // Try to read as UTF-8 text (metadata, YAML, JSON).
        if let Ok(s) = std::str::from_utf8(&buf) {
            if s.chars().any(|c| !c.is_control()) {
                text_parts.push(format!("# {}\n{}", entry_name, s));
            }
        }
    }

    let content = if text_parts.is_empty() {
        format!(
            "(No extractable text in this {} file. iWork native .iwa payloads require full protobuf schemas — see ADR 0006.)",
            format_label(format)
        )
    } else {
        text_parts.join("\n\n---\n\n")
    };

    Ok(Document::Text {
        content: format!(
            "[Partial preview — Apple iWork .iwa content not yet decoded (ADR 0003/0006)]\n\n{}",
            content
        ),
        encoding: "utf-8".into(),
        byte_len: bytes.len(),
        truncated: false,
    })
}

fn format_label(format: Format) -> &'static str {
    match format {
        Format::IworkPages => "Pages",
        Format::IworkNumbers => "Numbers",
        Format::IworkKey => "Keynote",
        _ => "iWork",
    }
}