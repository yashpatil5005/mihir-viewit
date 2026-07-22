//! ODT reader (OpenDocument Text).
//!
//! ODT is a zip containing `content.xml` with namespace
//! `urn:oasis:names:tc:opendocument:xmlns:office:1.0`.
//! We extract structured blocks (headings, paragraphs, list items) from
//! `<text:p>` and `<text:list>` elements in `<office:text>`.
//! Produces `Document::Docx` with structured blocks for the DocxViewer.

use std::io::{Cursor, Read};
use viewit_core_types::{DocxBlock, Document, Error, Format};
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
    let mut buf = Vec::new();
    let mut blocks: Vec<DocxBlock> = Vec::new();
    let mut in_text_p = false;
    let mut in_list_item = false;
    let mut _in_h = false;
    let mut current_text: Vec<String> = Vec::new();
    let mut heading_level: Option<u8> = None;
    let mut in_office_text = false;
    let mut in_text_list = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = e.name();
                let tag_ref = name.as_ref();

                // Detect <office:text> — everything inside is body content
                if tag_ref == b"office:text" {
                    in_office_text = true;
                }
                // Detect heading via text:style-name attribute on <text:p>
                // ODT headings use style names like "Heading_20_1", "Heading_20_2", etc.
                if tag_ref == b"text:p" && in_office_text {
                    in_text_p = true;
                    current_text.clear();
                    heading_level = None;

                    // Check for heading style
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"text:style-name" {
                            let val = String::from_utf8_lossy(&attr.value);
                            if val.starts_with("Heading_20_") {
                                if let Ok(level) = val.trim_start_matches("Heading_20_").parse::<u8>() {
                                    heading_level = Some(level);
                                    _in_h = true;
                                }
                            } else if val.starts_with("Heading") {
                                // Fallback: "Heading1", "Heading2", etc.
                                let rest = val.trim_start_matches("Heading");
                                if let Ok(level) = rest.parse::<u8>() {
                                    heading_level = Some(level);
                                    _in_h = true;
                                }
                            }
                        }
                    }
                }
                // Detect <text:list> — container for list items
                if tag_ref == b"text:list" && in_office_text {
                    in_text_list = true;
                }
                // Detect <text:list-item> — a list item starts
                if tag_ref == b"text:list-item" && in_text_list {
                    in_list_item = true;
                    current_text.clear();
                }
            }
            Ok(Event::Empty(e)) => {
                let name = e.name();
                let tag_ref = name.as_ref();
                // Detect empty <text:list-item/> (empty list item)
                if tag_ref == b"text:list-item" && in_text_list {
                    blocks.push(DocxBlock::ListItem {
                        text: String::new(),
                        level: 0,
                    });
                }
            }
            Ok(Event::Text(t)) => {
                if in_text_p || in_list_item {
                    let s = t.unescape().map(|s| s.into_owned()).unwrap_or_default();
                    current_text.push(s);
                }
            }
            Ok(Event::End(e)) => {
                let name = e.name();
                let tag_ref = name.as_ref();

                if tag_ref == b"text:p" && in_text_p {
                    in_text_p = false;
                    let text = current_text.concat();
                    if !text.trim().is_empty() {
                        blocks.push(DocxBlock::Paragraph {
                            text,
                            heading: heading_level,
                        });
                    }
                    heading_level = None;
                    _in_h = false;
                }
                if tag_ref == b"text:list-item" && in_list_item {
                    in_list_item = false;
                    let text = current_text.concat();
                    blocks.push(DocxBlock::ListItem {
                        text,
                        level: 0,
                    });
                }
                if tag_ref == b"text:list" {
                    in_text_list = false;
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
