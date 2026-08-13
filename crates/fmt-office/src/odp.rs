//! ODP / OTP parser (OpenDocument Presentation).
//!
//! ODP is a zip containing `content.xml` with presentation namespace.
//! We extract each `<draw:page>` and harvest text from `<text:p>` elements.
//! Produces Document::Pptx with structured slide data (title + body).

use std::io::{Cursor, Read};
use viewit_core_types::{Document, Error, Format, PptxElement, PptxSlide};
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
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut slides: Vec<PptxSlide> = Vec::new();
    let mut in_page = false;
    let mut in_notes = false;
    let mut in_text_p = false;
    let mut current_text: Vec<String> = Vec::new();
    let mut page_texts: Vec<String> = Vec::new();
    let mut page_elements: Vec<PptxElement> = Vec::new();
    let mut frame: Option<(u64, u64, u64, u64, Vec<String>)> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = e.name();
                let tag = name.local_name();
                let tag = tag.as_ref();
                if tag == b"page" {
                    in_page = true;
                    page_texts.clear();
                    page_elements.clear();
                }
                if tag == b"frame" && in_page && !in_notes {
                    frame = Some((
                        odf_length_attr(&e, b"x").unwrap_or(0),
                        odf_length_attr(&e, b"y").unwrap_or(0),
                        odf_length_attr(&e, b"width").unwrap_or(0),
                        odf_length_attr(&e, b"height").unwrap_or(0),
                        Vec::new(),
                    ));
                }
                // Skip speaker notes — they're not visible slide content.
                if tag == b"notes" && in_page {
                    in_notes = true;
                }
                if tag == b"p" && in_page && !in_notes {
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
                        if let Some((_, _, _, _, texts)) = &mut frame {
                            texts.push(text.clone());
                        }
                        page_texts.push(text);
                    }
                }
                if tag == b"frame" {
                    if let Some((x, y, w, h, texts)) = frame.take() {
                        let text = texts.join("\n");
                        if !text.is_empty() {
                            let index = page_elements.len();
                            let (x, y, w, h) = if w > 0 && h > 0 {
                                (x, y, w, h)
                            } else if index == 0 {
                                (640_080, 548_640, 7_863_840, 1_234_440)
                            } else {
                                (822_960, 2_057_400, 7_498_080, 3_977_640)
                            };
                            page_elements.push(PptxElement {
                                kind: "text".into(),
                                x,
                                y,
                                w,
                                h,
                                src: None,
                                text: Some(text),
                                font_size: Some(if index == 0 { 36.0 } else { 24.0 }),
                                paragraphs: None,
                            });
                        }
                    }
                }
                if tag == b"notes" && in_notes {
                    in_notes = false;
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
                        elements: page_elements.clone(),
                        // ODP default: 25.4cm x 19.05cm (10" x 7.5" in EMUs)
                        // matching the common 4:3 presentation size.
                        width: Some(9_144_000),
                        height: Some(6_858_000),
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

fn odf_length_attr(e: &quick_xml::events::BytesStart<'_>, local: &[u8]) -> Option<u64> {
    e.attributes().flatten().find_map(|attr| {
        if attr.key.local_name().as_ref() == local {
            odf_length_to_emu(&String::from_utf8_lossy(attr.value.as_ref()))
        } else {
            None
        }
    })
}

fn odf_length_to_emu(value: &str) -> Option<u64> {
    let split = value.find(|c: char| !c.is_ascii_digit() && c != '.' && c != '-')?;
    let amount = value[..split].parse::<f64>().ok()?;
    let unit = &value[split..];
    let emu = match unit {
        "cm" => amount * 360_000.0,
        "mm" => amount * 36_000.0,
        "in" => amount * 914_400.0,
        "pt" => amount * 12_700.0,
        _ => return None,
    };
    Some(emu.max(0.0).round() as u64)
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

    #[test]
    fn extracts_positioned_odp_frame() {
        let xml = r#"<office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0"><office:body><office:presentation><draw:page><draw:frame svg:x="1cm" svg:y="2cm" svg:width="10cm" svg:height="3cm"><draw:text-box><text:p>Placed text</text:p></draw:text-box></draw:frame></draw:page></office:presentation></office:body></office:document-content>"#;
        let slides = extract_odp_slides(xml);
        let element = &slides[0].elements[0];
        assert_eq!((element.x, element.y), (360_000, 720_000));
        assert_eq!((element.w, element.h), (3_600_000, 1_080_000));
        assert_eq!(element.text.as_deref(), Some("Placed text"));
    }
}
