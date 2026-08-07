//! PPTX / PPTM / POTX parser — unified for office-universal.
//!
//! Custom OOXML reader for presentation text extraction.
//! Produces Document::Pptx with slide list (title + body).

use std::io::{Cursor, Read};
use viewit_core_types::{Document, Error, Format, PptxSlide};
use zip::ZipArchive;

pub fn parse_pptx(bytes: &[u8], _format: Format, _name: &str) -> Result<Document, Error> {
    let cursor = Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor).map_err(|e| Error::Parse(format!("zip: {}", e)))?;

    let mut presentation_xml = String::new();
    if let Ok(mut f) = archive.by_name("ppt/presentation.xml") {
        f.read_to_string(&mut presentation_xml).ok();
    }

    let slide_count = count_slides_in_presentation(&presentation_xml);

    let mut slides: Vec<PptxSlide> = Vec::with_capacity(slide_count);
    for i in 1..=slide_count.min(200) {
        let slide_path = format!("ppt/slides/slide{}.xml", i);
        let mut slide_xml = String::new();
        if let Ok(mut f) = archive.by_name(&slide_path) {
            f.read_to_string(&mut slide_xml).ok();
        }
        let (title, body) = extract_slide_text(&slide_xml);
        slides.push(PptxSlide {
            title,
            body,
            elements: Vec::new(),
        });
    }

    Ok(Document::Pptx {
        slide_count,
        slides,
        byte_len: bytes.len(),
        asset_path: String::new(),
        stream_url: None,
    })
}

fn count_slides_in_presentation(xml: &str) -> usize {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    let mut count = 0;
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                if e.name().local_name().as_ref() == b"sldIdLst" {
                    loop {
                        match reader.read_event_into(&mut buf) {
                            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                                if e.name().local_name().as_ref() == b"sldId" {
                                    count += 1;
                                }
                            }
                            Ok(Event::End(e)) if e.name().local_name().as_ref() == b"sldIdLst" => {
                                break
                            }
                            Ok(Event::Eof) => break,
                            _ => {}
                        }
                        buf.clear();
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    count
}

fn extract_slide_text(xml: &str) -> (String, String) {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut title = String::new();
    let mut body = String::new();
    let mut in_title = false;
    let mut in_body = false;
    let mut current_text = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let name = e.name();
                for attr in e.attributes().flatten() {
                    if attr.key.as_ref() == b"type" {
                        let val = attr.value.as_ref();
                        if val == b"ctrTitle" || val == b"title" {
                            in_title = true;
                        } else if val == b"body" {
                            in_body = true;
                        }
                    }
                }
                if name.local_name().as_ref() == b"t" {
                    current_text.clear();
                }
            }
            Ok(Event::Text(t)) => {
                let s = t.unescape().map(|s| s.into_owned()).unwrap_or_default();
                current_text.push_str(&s);
            }
            Ok(Event::End(e)) => {
                if e.name().local_name().as_ref() == b"t" {
                    if in_title && title.is_empty() {
                        title = current_text.clone();
                    } else if in_body {
                        if !body.is_empty() {
                            body.push('\n');
                        }
                        body.push_str(&current_text);
                    }
                    current_text.clear();
                }
                if e.name().local_name().as_ref() == b"sp"
                    || e.name().local_name().as_ref() == b"txBody"
                {
                    in_title = false;
                    in_body = false;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    (title, body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_slide_text_basic() {
        let xml = r#"
        <p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
          <p:cSld><p:spTree>
            <p:sp><p:nvSpPr><p:ph type="title"/></p:nvSpPr><p:txBody><a:p><a:t>Slide Title</a:t></a:p></p:txBody></p:sp>
            <p:sp><p:nvSpPr><p:ph type="body"/></p:nvSpPr><p:txBody><a:p><a:t>Body text</a:t></a:p></p:txBody></p:sp>
          </p:spTree></p:cSld>
        </p:sld>
        "#;
        let (title, body) = extract_slide_text(xml);
        assert_eq!(title, "Slide Title");
        assert_eq!(body, "Body text");
    }
}
