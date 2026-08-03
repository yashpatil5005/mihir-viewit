//! ODT / OTT parser (OpenDocument Text).
//!
//! ODT is a zip containing `content.xml` with namespace
//! `urn:oasis:names:tc:opendocument:xmlns:office:1.0`.
//! We extract structured blocks (headings, paragraphs, list items, tables, and
//! image placeholders) from `<office:text>`.
//! Produces Document::Docx with structured blocks for the DocxViewer.

use std::io::{Cursor, Read};
use viewit_core_types::{Document, DocxBlock, Error, Format};
use zip::ZipArchive;

pub fn parse_odt(bytes: &[u8], _format: Format, _name: &str) -> Result<Document, Error> {
    let cursor = Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor).map_err(|e| Error::Parse(format!("zip: {}", e)))?;

    let mut content_xml = String::new();
    match archive.by_name("content.xml") {
        Ok(mut f) => f.read_to_string(&mut content_xml).ok(),
        Err(_) => {
            return Ok(Document::Text {
                content: "(ODT archive with no content.xml)".into(),
                encoding: "utf-8".into(),
                truncated: false,
                byte_len: bytes.len(),
                stream_url: None,
            })
        }
    };

    let blocks = extract_odt_blocks(&content_xml);

    Ok(Document::Docx {
        blocks,
        byte_len: bytes.len(),
    })
}

fn extract_odt_blocks(xml: &str) -> Vec<DocxBlock> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut blocks: Vec<DocxBlock> = Vec::new();
    let mut in_paragraph = false;
    let mut in_list_item = false;
    let mut current_text = String::new();
    let mut heading_level: Option<u8> = None;
    let mut in_office_text = false;
    let mut in_text_list = false;
    let mut list_level: u8 = 0;
    let mut in_table = false;
    let mut in_table_cell = false;
    let mut current_row: Vec<String> = Vec::new();
    let mut table_rows: Vec<Vec<String>> = Vec::new();
    let mut cell_text = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = e.name();
                let tag_ref = name.as_ref();

                if tag_ref == b"office:text" {
                    in_office_text = true;
                }
                if tag_ref == b"table:table" && in_office_text {
                    in_table = true;
                    table_rows.clear();
                }
                if tag_ref == b"table:table-row" && in_table {
                    current_row.clear();
                }
                if tag_ref == b"table:table-cell" && in_table {
                    in_table_cell = true;
                    cell_text.clear();
                }
                if (tag_ref == b"text:p" || tag_ref == b"text:h") && in_office_text {
                    in_paragraph = true;
                    current_text.clear();
                    heading_level = if tag_ref == b"text:h" { Some(1) } else { None };

                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"text:outline-level" {
                            let val = String::from_utf8_lossy(attr.value.as_ref());
                            if let Ok(level) = val.parse::<u8>() {
                                heading_level = Some(level);
                            }
                        } else if attr.key.as_ref() == b"text:style-name" {
                            let val = String::from_utf8_lossy(&attr.value);
                            if let Some(level) = heading_from_style(&val) {
                                heading_level = Some(level);
                            }
                        }
                    }
                }
                if tag_ref == b"text:list" && in_office_text {
                    in_text_list = true;
                    list_level = list_level.saturating_add(1);
                }
                if tag_ref == b"text:list-item" && in_text_list {
                    in_list_item = true;
                    current_text.clear();
                }
                if tag_ref == b"draw:image" && in_office_text && !in_table_cell {
                    blocks.push(DocxBlock::Image {
                        name: image_name(&e),
                        src: None,
                    });
                }
            }
            Ok(Event::Empty(e)) => {
                let name = e.name();
                let tag_ref = name.as_ref();
                if tag_ref == b"text:list-item" && in_text_list {
                    blocks.push(DocxBlock::ListItem {
                        text: String::new(),
                        level: list_level.saturating_sub(1),
                    });
                }
                if tag_ref == b"draw:image" && in_office_text {
                    if in_table_cell {
                        if !cell_text.is_empty() {
                            cell_text.push(' ');
                        }
                        cell_text.push_str("[Image]");
                    } else {
                        blocks.push(DocxBlock::Image {
                            name: image_name(&e),
                            src: None,
                        });
                    }
                }
            }
            Ok(Event::Text(t)) => {
                let s = t.unescape().map(|s| s.into_owned()).unwrap_or_default();
                if in_table_cell {
                    cell_text.push_str(&s);
                } else if in_paragraph || in_list_item {
                    current_text.push_str(&s);
                }
            }
            Ok(Event::End(e)) => {
                let name = e.name();
                let tag_ref = name.as_ref();

                if (tag_ref == b"text:p" || tag_ref == b"text:h") && in_paragraph {
                    if !in_table && !in_list_item {
                        let text = current_text.trim().to_string();
                        if !text.is_empty() {
                            blocks.push(DocxBlock::Paragraph {
                                text,
                                heading: heading_level,
                            });
                        }
                    }
                    in_paragraph = false;
                    heading_level = None;
                }
                if tag_ref == b"text:list-item" && in_list_item {
                    in_list_item = false;
                    if !in_table {
                        let text = current_text.trim().to_string();
                        if !text.is_empty() {
                            blocks.push(DocxBlock::ListItem {
                                text,
                                level: list_level.saturating_sub(1),
                            });
                        }
                    }
                }
                if tag_ref == b"text:list" {
                    list_level = list_level.saturating_sub(1);
                    in_text_list = false;
                }
                if tag_ref == b"table:table-cell" && in_table {
                    current_row.push(cell_text.trim().to_string());
                    in_table_cell = false;
                }
                if tag_ref == b"table:table-row" && in_table {
                    if !current_row.is_empty() {
                        table_rows.push(current_row.clone());
                    }
                }
                if tag_ref == b"table:table" && in_table {
                    if !table_rows.is_empty() {
                        blocks.push(DocxBlock::Table {
                            rows: table_rows.clone(),
                        });
                    }
                    in_table = false;
                }
                if tag_ref == b"office:text" {
                    in_office_text = false;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    blocks
}

fn heading_from_style(style: &str) -> Option<u8> {
    if style == "Title" {
        return Some(1);
    }
    style
        .strip_prefix("Heading_20_")
        .or_else(|| style.strip_prefix("Heading"))
        .or_else(|| style.strip_prefix("heading"))
        .and_then(|level| level.trim().parse::<u8>().ok())
}

fn image_name(e: &quick_xml::events::BytesStart<'_>) -> String {
    e.attributes()
        .flatten()
        .find_map(|attr| {
            if attr.key.as_ref() == b"xlink:href" {
                Some(String::from_utf8_lossy(attr.value.as_ref()).to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "Embedded image".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_odt_structure() {
        let xml = r#"
        <office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" xmlns:xlink="http://www.w3.org/1999/xlink">
          <office:body><office:text>
            <text:h text:outline-level="1">Heading</text:h>
            <text:p>Paragraph</text:p>
            <text:list><text:list-item><text:p>Item</text:p></text:list-item></text:list>
            <table:table><table:table-row><table:table-cell><text:p>A1</text:p></table:table-cell><table:table-cell><text:p>B1</text:p></table:table-cell></table:table-row></table:table>
            <draw:image xlink:href="Pictures/pic.png" />
          </office:text></office:body>
        </office:document-content>
        "#;

        let blocks = extract_odt_blocks(xml);
        assert!(matches!(
            blocks[0],
            DocxBlock::Paragraph {
                heading: Some(1),
                ..
            }
        ));
        assert!(matches!(blocks[2], DocxBlock::ListItem { .. }));
        assert!(
            matches!(&blocks[3], DocxBlock::Table { rows } if rows == &vec![vec!["A1".to_string(), "B1".to_string()]])
        );
        assert!(matches!(&blocks[4], DocxBlock::Image { name, .. } if name == "Pictures/pic.png"));
    }
}