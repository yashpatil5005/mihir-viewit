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
use viewit_core_types::{DocxBlock, Document, Error, Format, PptxSlide, XlsxSheet};
use zip::ZipArchive;

pub fn parse(bytes: &[u8], format: Format, _name: &str) -> Result<Document, Error> {
    let cursor = Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor).map_err(|e| Error::Parse(format!("zip: {}", e)))?;

    let mut text_parts: Vec<(String, String)> = Vec::new();
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
                text_parts.push((entry_name, s.to_string()));
            }
        }
    }

    Ok(match format {
        Format::IworkPages => Document::Docx {
            blocks: iwork_pages_blocks(&text_parts, preview_jpg.is_some()),
            byte_len: bytes.len(),
        },
        Format::IworkNumbers => Document::Xlsx {
            sheets: iwork_numbers_sheets(&text_parts),
            byte_len: bytes.len(),
        },
        Format::IworkKey => Document::Pptx {
            slide_count: 1,
            slides: vec![PptxSlide {
                title: "Apple Keynote preview".into(),
                body: iwork_text_body(format, &text_parts, preview_jpg.is_some()),
            }],
            byte_len: bytes.len(),
            asset_path: String::new(),
            stream_url: None,
        },
        _ => Document::Text {
            content: iwork_text_body(format, &text_parts, preview_jpg.is_some()),
            encoding: "utf-8".into(),
            byte_len: bytes.len(),
            truncated: false,
            stream_url: None,
        },
    })
}

fn iwork_pages_blocks(text_parts: &[(String, String)], has_preview: bool) -> Vec<DocxBlock> {
    let mut blocks = vec![DocxBlock::Paragraph {
        text: partial_notice(has_preview),
        heading: Some(1),
    }];
    for (name, text) in text_parts.iter().filter(|(name, _)| !is_metadata(name)) {
        blocks.push(DocxBlock::Paragraph {
            text: name.clone(),
            heading: Some(2),
        });
        for paragraph in text.split('\n').map(str::trim).filter(|s| !s.is_empty()) {
            blocks.push(DocxBlock::Paragraph {
                text: paragraph.to_string(),
                heading: None,
            });
        }
    }
    if blocks.len() == 1 {
        blocks.push(DocxBlock::Paragraph {
            text: no_extractable_text(Format::IworkPages),
            heading: None,
        });
    }
    blocks
}

fn iwork_numbers_sheets(text_parts: &[(String, String)]) -> Vec<XlsxSheet> {
    let mut rows = Vec::new();
    for (name, text) in text_parts.iter().filter(|(name, _)| !is_metadata(name)) {
        rows.push(vec![name.clone(), String::new()]);
        for line in text.lines().map(str::trim).filter(|s| !s.is_empty()) {
            let cols: Vec<String> = if line.contains('\t') {
                line.split('\t').map(|s| s.trim().to_string()).collect()
            } else if line.contains(',') {
                line.split(',').map(|s| s.trim().to_string()).collect()
            } else {
                vec![line.to_string()]
            };
            rows.push(cols);
        }
    }
    if rows.is_empty() {
        rows.push(vec![no_extractable_text(Format::IworkNumbers)]);
    }
    let total_rows = rows.len() + 1;
    let total_cols = rows.iter().map(Vec::len).max().unwrap_or(1).max(2);
    vec![XlsxSheet {
        name: "iWork preview".into(),
        header: vec![partial_notice(false), "".into()],
        preview_rows: rows,
        total_rows_hint: Some(total_rows),
        total_cols_hint: Some(total_cols),
    }]
}

fn iwork_text_body(format: Format, text_parts: &[(String, String)], has_preview: bool) -> String {
    let extracted: Vec<String> = text_parts
        .iter()
        .filter(|(name, _)| !is_metadata(name))
        .map(|(name, text)| format!("{}\n{}", name, text.trim()))
        .collect();
    if extracted.is_empty() {
        format!("{}\n\n{}", partial_notice(has_preview), no_extractable_text(format))
    } else {
        format!("{}\n\n{}", partial_notice(has_preview), extracted.join("\n\n---\n\n"))
    }
}

fn partial_notice(has_preview: bool) -> String {
    if has_preview {
        "Partial iWork preview: native .iwa layout is not decoded yet; preview image/text metadata were preserved.".into()
    } else {
        "Partial iWork preview: native .iwa layout is not decoded yet; extractable package text is rendered structurally.".into()
    }
}

fn no_extractable_text(format: Format) -> String {
    format!(
        "No extractable text in this {} file. iWork native .iwa payloads require full protobuf schemas.",
        format_label(format)
    )
}

fn is_metadata(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.ends_with("metadata.json") || lower.ends_with("document.yaml")
}

fn format_label(format: Format) -> &'static str {
    match format {
        Format::IworkPages => "Pages",
        Format::IworkNumbers => "Numbers",
        Format::IworkKey => "Keynote",
        _ => "iWork",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn iwork_zip(text_path: &str, text: &str) -> Vec<u8> {
        let mut buf = Vec::new();
        let cursor = std::io::Cursor::new(&mut buf);
        let mut zip = zip::ZipWriter::new(cursor);
        zip.start_file("Metadata.json", zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(b"{}\n").unwrap();
        zip.start_file(text_path, zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(text.as_bytes()).unwrap();
        zip.finish().unwrap();
        buf
    }

    #[test]
    fn pages_maps_to_structured_docx() {
        let bytes = iwork_zip("Data/document.txt", "Hello Pages\n");
        let doc = parse(&bytes, Format::IworkPages, "sample.pages").unwrap();
        assert!(matches!(doc, Document::Docx { blocks, .. } if blocks.iter().any(|b| matches!(b, DocxBlock::Paragraph { text, .. } if text == "Hello Pages"))));
    }

    #[test]
    fn numbers_maps_to_structured_grid() {
        let bytes = iwork_zip("Data/table.tsv", "A\tB\n1\t2\n");
        let doc = parse(&bytes, Format::IworkNumbers, "sample.numbers").unwrap();
        assert!(matches!(doc, Document::Xlsx { sheets, .. } if sheets[0].preview_rows.iter().any(|r| r == &vec!["A".to_string(), "B".to_string()])));
    }

    #[test]
    fn key_maps_to_structured_deck() {
        let bytes = iwork_zip("Data/document.txt", "Hello Keynote\n");
        let doc = parse(&bytes, Format::IworkKey, "sample.key").unwrap();
        assert!(matches!(doc, Document::Pptx { slides, .. } if slides[0].body.contains("Hello Keynote")));
    }
}
