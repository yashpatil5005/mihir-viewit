//! Phase 3.4 — ODT reader (OpenDocument Text).
//!
//! ODT is a zip containing `content.xml` with namespace
//! `urn:oasis:names:tc:opendocument:xmlns:office:1.0`.
//! We extract text from `<text:p>` elements in `<office:text>`.
//! Produces `Document::Text`.

use std::io::{Cursor, Read};
use viewit_core_types::{Document, Error, Format};
use zip::ZipArchive;

pub fn parse_odt(bytes: &[u8], _format: Format, _name: &str) -> Result<Document, Error> {
    let cursor = Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor).map_err(|e| Error::Parse(format!("zip: {}", e)))?;

    let mut content_xml = String::new();
    match archive.by_name("content.xml") {
        Ok(mut f) => f.read_to_string(&mut content_xml).ok(),
        Err(_) => return Ok(Document::Text {
            content: "(ODT archive with no content.xml)".into(),
            encoding: "utf-8".into(),
            byte_len: bytes.len(),
        }),
    };

    let text = extract_odt_text(&content_xml);

    Ok(Document::Text {
        content: text,
        encoding: "utf-8".into(),
        byte_len: bytes.len(),
    })
}

fn extract_odt_text(xml: &str) -> String {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    let mut out = String::new();
    let mut in_text = false;
    let mut para_text: Vec<String> = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                if e.name().as_ref() == b"text:p" {
                    in_text = true;
                    para_text.clear();
                }
            }
            Ok(Event::Empty(_)) => {}
            Ok(Event::Text(t)) => {
                if in_text {
                    let s = t.unescape().map(|s| s.into_owned()).unwrap_or_default();
                    para_text.push(s);
                }
            }
            Ok(Event::End(e)) => {
                if e.name().as_ref() == b"text:p" {
                    in_text = false;
                    if !para_text.is_empty() {
                        if !out.is_empty() {
                            out.push_str("\n\n");
                        }
                        out.push_str(&para_text.concat());
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