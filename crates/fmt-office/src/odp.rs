//! ODP / OTP parser (OpenDocument Presentation).
//!
//! ODP is a zip containing `content.xml` with presentation namespace.
//! We extract each `<draw:page>` and harvest text from `<text:p>` elements.
//! Produces Document::Pptx with structured slide data (title + body).

use std::io::{Cursor, Read};
use viewit_core_types::{Document, Error, Format, PptxSlide};
use zip::ZipArchive;

pub fn parse_odp(bytes: &[u8], _format: Format, _name: &str) -> Result<Document, Error> {
    let cursor = Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor).map_err(|e| Error::Parse(format!("zip: {}", e)))?;

    let mut content_xml = String::new();
    match archive.by_name("content.xml") {
        Ok(mut f) => f.read_to_string(&mut content_xml).ok(),
        Err(_) => {
            return Ok(Document::Text {
                content: "(ODP archive with no content.xml)".into(),
                encoding: "utf-8".into(),
                truncated: false,
                byte_len: bytes.len(),
                stream_url: None,
            })
        }
    };

    let slides = extract_odp_slides(&content_xml);

    Ok(Document::Pptx {
        slide_count: slides.len(),
        slides,
        byte_len: bytes.len(),
        asset_path: String::new(),
        stream_url: None,
    })
}

fn extract_odp_slides(xml: &str) -> Vec<PptxSlide> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut slides: Vec<PptxSlide> = Vec::new();
    let mut in_page = false;
    let mut in_text_p = false;
    let mut current_text: Vec<String> = Vec::new();
    let mut page_texts: Vec<String> = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = e.name();
                let tag = name.local_name();
                let tag = tag.as_ref();
                if tag == b"page" {
                    in_page = true;
                    page_texts.clear();
                }
                if tag == b"p" && in_page {
                    in_text_p = true;
                    current_text.clear();
                }
            }
            Ok(Event::Text(t)) => {
                if in_text_p {
                    let s = t.unescape().map(|s| s.into_owned()).unwrap_or_default();
                    current_text.push(s);
                }
            }
            Ok(Event::End(e)) => {
                let name = e.name();
                let tag = name.local_name();
                let tag = tag.as_ref();
                if tag == b"p" && in_text_p {
                    in_text_p = false;
                    let text = current_text.concat();
                    if !text.trim().is_empty() {
                        page_texts.push(text);
                    }
                }
                if tag == b"page" && in_page {
                    in_page = false;
                    // First text paragraph is typically the title; rest is body
                    let title = page_texts.first().cloned().unwrap_or_default();
                    let body = if page_texts.len() > 1 {
                        page_texts[1..].join("\n")
                    } else {
                        String::new()
                    };
                    slides.push(PptxSlide {
                        title,
                        body,
                        elements: Vec::new(),
                        width: None,
                        height: None,
                        background: None,
                    });
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    slides
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_odp_slides() {
        let xml = r#"
        <office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0">
          <office:body>
            <office:presentation>
              <draw:page draw:name="slide1">
                <text:p>Slide Title</text:p>
                <text:p>Body text line 1</text:p>
                <text:p>Body text line 2</text:p>
              </draw:page>
              <draw:page draw:name="slide2">
                <text:p>Another Slide</text:p>
                <text:p>More content</text:p>
              </draw:page>
            </office:presentation>
          </office:body>
        </office:document-content>
        "#;

        let slides = extract_odp_slides(xml);
        assert_eq!(slides.len(), 2);
        assert_eq!(slides[0].title, "Slide Title");
        assert_eq!(slides[0].body, "Body text line 1\nBody text line 2");
        assert_eq!(slides[1].title, "Another Slide");
        assert_eq!(slides[1].body, "More content");
    }
}
