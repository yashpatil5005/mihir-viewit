//! Phase 3.4 — ODP reader (OpenDocument Presentation).
//!
//! ODP is a zip containing `content.xml` with presentation namespace.
//! We extract each `<draw:page>` and harvest text from `<text:p>` elements.
//! Produces `Document::Text` with slide text blocks.

use std::io::{Cursor, Read};
use viewit_core_types::{Document, Error, Format};
use zip::ZipArchive;

pub fn parse_odp(bytes: &[u8], _format: Format, _name: &str) -> Result<Document, Error> {
    let cursor = Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor).map_err(|e| Error::Parse(format!("zip: {}", e)))?;

    let mut content_xml = String::new();
    match archive.by_name("content.xml") {
        Ok(mut f) => f.read_to_string(&mut content_xml).ok(),
        Err(_) => return Ok(Document::Text {
            content: "(ODP archive with no content.xml)".into(),
            encoding: "utf-8".into(),
            truncated: false,
            byte_len: bytes.len(),
        }),
    };

    let text = extract_odp_text(&content_xml);

    Ok(Document::Text {
        content: text,
        encoding: "utf-8".into(),
        byte_len: bytes.len(),
        truncated: false,
    })
}

fn extract_odp_text(xml: &str) -> String {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    let mut out = String::new();
    let mut in_text = false;
    let mut page_text: Vec<String> = Vec::new();
    let mut page_count = 0;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                if e.name().as_ref() == b"text:p" {
                    in_text = true;
                    page_text.clear();
                }
            }
            Ok(Event::Empty(_)) => {}
            Ok(Event::Text(t)) => {
                if in_text {
                    let s = t.unescape().map(|s| s.into_owned()).unwrap_or_default();
                    page_text.push(s);
                }
            }
            Ok(Event::End(e)) => {
                if e.name().as_ref() == b"text:p" {
                    in_text = false;
                    if !page_text.is_empty() {
                        if !out.is_empty() {
                            out.push_str("\n\n");
                        }
                        out.push_str(&page_text.concat());
                    }
                }
                if e.name().as_ref() == b"draw:page" {
                    page_count += 1;
                    if !out.is_empty() {
                        out.push_str(&format!("\n\n--- Slide {} ---", page_count));
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    out
}