//! PPTX / PPTM / POTX parser — unified for office-universal.
//!
//! Produces `Document::Pptx` with a real slide viewport: slide extents + solid
//! background, and positioned elements (text boxes with styled runs, images)
//! so viewers render the deck's actual layout instead of a flat text dump.
//! Rich-parse logic mirrors the `office-ooxml` catalog plugin.

use base64::Engine as _;
use std::io::{Cursor, Read};
use viewit_core_types::{
    Document, Error, Format, PptxBackground, PptxElement, PptxParagraph, PptxRun, PptxSlide,
};
use zip::ZipArchive;

const DEFAULT_SLIDE_W: u64 = 9_144_000;
const DEFAULT_SLIDE_H: u64 = 5_143_500;

pub fn parse_pptx(bytes: &[u8], _format: Format, _name: &str) -> Result<Document, Error> {
    let cursor = Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor).map_err(|e| Error::Parse(format!("zip: {e}")))?;

    let mut presentation_xml = String::new();
    if let Ok(mut f) = archive.by_name("ppt/presentation.xml") {
        f.read_to_string(&mut presentation_xml).ok();
    }
    let slide_count = count_slides_in_presentation(&presentation_xml);
    let (slide_w, slide_h) =
        parse_presentation_size(bytes).unwrap_or((DEFAULT_SLIDE_W, DEFAULT_SLIDE_H));

    let mut slides: Vec<PptxSlide> = Vec::with_capacity(slide_count.min(200));
    for i in 1..=slide_count.min(200) {
        let slide_path = format!("ppt/slides/slide{}.xml", i);
        let slide_xml = zip_entry_string(bytes, &slide_path).unwrap_or_default();

        let (title, body) = extract_slide_text(&slide_xml);
        let rels = slide_relationships(bytes, i).unwrap_or_default();
        let elements = parse_slide_elements(bytes, &slide_xml, &rels, slide_w, slide_h);
        let background = parse_slide_background(&slide_xml);

        slides.push(PptxSlide {
            title,
            body,
            width: Some(slide_w),
            height: Some(slide_h),
            elements,
            background,
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
    let mut all_texts: Vec<String> = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                if e.name().local_name().as_ref() == b"t" {
                    current_text.clear();
                }
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
            }
            Ok(Event::Text(t)) => {
                let s = t.unescape().map(|s| s.into_owned()).unwrap_or_default();
                current_text.push_str(&s);
            }
            Ok(Event::End(e)) => {
                if e.name().local_name().as_ref() == b"t" {
                    let text = std::mem::take(&mut current_text);
                    if !text.trim().is_empty() {
                        all_texts.push(text.trim().to_string());
                    }
                    if in_title && title.is_empty() {
                        title = text.clone();
                    } else if in_body {
                        if !body.is_empty() {
                            body.push('\n');
                        }
                        body.push_str(&text);
                    }
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

    if !all_texts.is_empty() && title.is_empty() {
        let mut it = all_texts.into_iter();
        title = it.next().unwrap_or_default();
        body = it.collect::<Vec<_>>().join("\n");
    }

    (title, body)
}

fn parse_presentation_size(bytes: &[u8]) -> Option<(u64, u64)> {
    let xml = zip_entry_string(bytes, "ppt/presentation.xml").ok()?;
    let mut reader = quick_xml::Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(quick_xml::events::Event::Start(e)) | Ok(quick_xml::events::Event::Empty(e))
                if e.name().as_ref() == b"p:sldSz" =>
            {
                let w = x_attr(&e, b"cx")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(DEFAULT_SLIDE_W);
                let h = x_attr(&e, b"cy")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(DEFAULT_SLIDE_H);
                return Some((w, h));
            }
            Ok(quick_xml::events::Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    None
}

fn slide_relationships(
    bytes: &[u8],
    index: usize,
) -> Result<std::collections::HashMap<String, String>, Error> {
    let path = format!("ppt/slides/_rels/slide{index}.xml.rels");
    let rels_xml = zip_entry_string(bytes, &path)?;
    let mut rels = std::collections::HashMap::new();
    let mut reader = quick_xml::Reader::from_str(&rels_xml);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(quick_xml::events::Event::Start(e)) | Ok(quick_xml::events::Event::Empty(e))
                if e.name().as_ref() == b"Relationship" =>
            {
                if let (Some(id), Some(target)) = (x_attr(&e, b"Id"), x_attr(&e, b"Target")) {
                    rels.insert(id, target);
                }
            }
            Ok(quick_xml::events::Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    Ok(rels)
}

// ---- Element + background parsing (port of office-ooxml rich slide model) ----

fn parse_slide_background(xml: &str) -> Option<PptxBackground> {
    use quick_xml::events::Event;

    let mut reader = quick_xml::Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut bg = None;
    let mut in_bg = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if e.name().as_ref() == b"p:bg" => in_bg = true,
            Ok(Event::End(e)) if e.name().as_ref() == b"p:bg" => in_bg = false,
            Ok(Event::Empty(e)) if in_bg && e.name().as_ref() == b"a:srgbClr" => {
                if let Some(color) = x_attr(&e, b"val") {
                    bg = Some(PptxBackground {
                        kind: "solid".into(),
                        color: Some(format!("#{color}")),
                        gradient: None,
                    });
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    bg
}

struct ElementBuilder {
    kind: &'static str,
    name: String,
    x: i64,
    y: i64,
    w: i64,
    h: i64,
    text: String,
    rel_id: String,
    font_size: Option<f64>,
    paragraphs: Vec<PptxParagraph>,
    current_paragraph: PptxParagraph,
}

impl Default for ElementBuilder {
    fn default() -> Self {
        Self {
            kind: "text",
            name: String::new(),
            x: 0,
            y: 0,
            w: 0,
            h: 0,
            text: String::new(),
            rel_id: String::new(),
            font_size: None,
            paragraphs: Vec::new(),
            current_paragraph: PptxParagraph::default(),
        }
    }
}

#[derive(Default)]
struct RunAccum {
    text: String,
    bold: bool,
    italic: bool,
    underline: bool,
    font_size: Option<f64>,
    color: Option<String>,
}

fn parse_slide_elements(
    bytes: &[u8],
    xml: &str,
    rels: &std::collections::HashMap<String, String>,
    slide_w: u64,
    slide_h: u64,
) -> Vec<PptxElement> {
    use quick_xml::events::Event;

    let mut reader = quick_xml::Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut elements: Vec<PptxElement> = Vec::new();
    let mut current: Option<ElementBuilder> = None;
    let mut run_accum: Option<RunAccum> = None;
    let mut in_text = false;
    let mut in_rpr = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if e.name().as_ref() == b"p:sp" => {
                current = Some(ElementBuilder {
                    kind: "text",
                    ..Default::default()
                });
            }
            Ok(Event::Start(e)) if e.name().as_ref() == b"p:pic" => {
                current = Some(ElementBuilder {
                    kind: "image",
                    ..Default::default()
                });
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if e.name().as_ref() == b"p:cNvPr" => {
                if let Some(item) = &mut current {
                    item.name = x_attr(&e, b"name").unwrap_or_default();
                }
            }
            Ok(Event::Empty(e)) if e.name().as_ref() == b"a:off" => {
                if let Some(item) = &mut current {
                    item.x = x_attr(&e, b"x").and_then(|v| v.parse().ok()).unwrap_or(0);
                    item.y = x_attr(&e, b"y").and_then(|v| v.parse().ok()).unwrap_or(0);
                }
            }
            Ok(Event::Empty(e)) if e.name().as_ref() == b"a:ext" => {
                if let Some(item) = &mut current {
                    item.w = x_attr(&e, b"cx").and_then(|v| v.parse().ok()).unwrap_or(0);
                    item.h = x_attr(&e, b"cy").and_then(|v| v.parse().ok()).unwrap_or(0);
                }
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if e.name().as_ref() == b"a:rPr" => {
                in_rpr = true;
                let font_size = x_attr(&e, b"sz")
                    .and_then(|v| v.parse::<f64>().ok())
                    .map(|v| v / 100.0);
                if let Some(item) = &mut current {
                    if item.font_size.is_none() {
                        item.font_size = font_size;
                    }
                }
                if let Some(acc) = &mut run_accum {
                    if acc.font_size.is_none() {
                        acc.font_size = font_size;
                    }
                    if e.attributes()
                        .flatten()
                        .any(|a| a.key.as_ref() == b"b" && a.value.as_ref() == b"1")
                    {
                        acc.bold = true;
                    }
                    if e.attributes()
                        .flatten()
                        .any(|a| a.key.as_ref() == b"i" && a.value.as_ref() == b"1")
                    {
                        acc.italic = true;
                    }
                    if e.attributes()
                        .flatten()
                        .any(|a| a.key.as_ref() == b"u" && a.value.as_ref() == b"sng")
                    {
                        acc.underline = true;
                    }
                }
            }
            Ok(Event::Empty(e)) if e.name().as_ref() == b"a:blip" => {
                if let Some(item) = &mut current {
                    item.rel_id = x_attr(&e, b"r:embed").unwrap_or_default();
                }
            }
            Ok(Event::Empty(e)) if in_rpr && e.name().as_ref() == b"a:srgbClr" => {
                if let Some(val) = x_attr(&e, b"val") {
                    if let Some(acc) = &mut run_accum {
                        if acc.color.is_none() {
                            acc.color = Some(format!("#{}", val));
                        }
                    }
                }
            }
            Ok(Event::Start(e))
                if e.name().as_ref() == b"a:buChar" || e.name().as_ref() == b"a:buNone" =>
            {
                if let Some(item) = &mut current {
                    item.current_paragraph.bullet = true;
                }
            }
            Ok(Event::Start(e)) if e.name().as_ref() == b"a:p" => {
                if let Some(item) = &mut current {
                    item.current_paragraph = PptxParagraph::default();
                }
            }
            Ok(Event::Start(e)) if e.name().as_ref() == b"a:r" => {
                run_accum = Some(RunAccum::default());
            }
            Ok(Event::Start(e)) if e.name().as_ref() == b"a:t" => in_text = true,
            Ok(Event::Text(t)) if in_text => {
                if let Ok(decoded) = t.unescape() {
                    if let Some(acc) = &mut run_accum {
                        acc.text.push_str(&decoded);
                    } else if let Some(item) = &mut current {
                        item.text.push_str(&decoded);
                    }
                }
            }
            Ok(Event::End(e)) if e.name().as_ref() == b"a:t" => in_text = false,
            Ok(Event::End(e)) if e.name().as_ref() == b"a:rPr" => in_rpr = false,
            Ok(Event::End(e)) if e.name().as_ref() == b"a:r" => {
                if let Some(acc) = run_accum.take() {
                    if let Some(item) = &mut current {
                        if !acc.text.is_empty() {
                            item.current_paragraph.runs.push(PptxRun {
                                text: acc.text,
                                bold: acc.bold,
                                italic: acc.italic,
                                underline: acc.underline,
                                font_size: acc.font_size,
                                color: acc.color,
                            });
                        }
                    }
                }
            }
            Ok(Event::End(e)) if e.name().as_ref() == b"a:p" => {
                if let Some(item) = &mut current {
                    let para = std::mem::take(&mut item.current_paragraph);
                    if !para.runs.is_empty() || para.bullet {
                        item.paragraphs.push(para);
                    }
                }
            }
            Ok(Event::End(e)) if e.name().as_ref() == b"p:sp" || e.name().as_ref() == b"p:pic" => {
                if let Some(item) = current.take() {
                    if let Some(element) = finish_element(bytes, item, rels, slide_w, slide_h) {
                        elements.push(element);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }

    elements
}

fn finish_element(
    bytes: &[u8],
    item: ElementBuilder,
    rels: &std::collections::HashMap<String, String>,
    slide_w: u64,
    slide_h: u64,
) -> Option<PptxElement> {
    let sw = slide_w as i64;
    let sh = slide_h as i64;
    if item.kind == "text" {
        let mut flat = String::new();
        for para in &item.paragraphs {
            if para.runs.is_empty() {
                if para.bullet {
                    flat.push_str("• ");
                    flat.push('\n');
                }
            } else {
                for run in &para.runs {
                    flat.push_str(&run.text);
                }
                flat.push('\n');
            }
        }
        let text = if flat.trim().is_empty() {
            item.text.trim().to_string()
        } else {
            flat.trim_end().to_string()
        };
        if text.is_empty() {
            return None;
        }
        let (x, y, w, h) = element_box(&item, &text, sw, sh);
        if w == 0 || h == 0 {
            return None;
        }
        let font_size = item
            .font_size
            .unwrap_or_else(|| fallback_font_size(&item.name, &text));
        return Some(PptxElement {
            kind: "text".into(),
            x: x.clamp(0, sw) as u64,
            y: y.clamp(0, sh) as u64,
            w: w.clamp(0, sw) as u64,
            h: h.clamp(0, sh) as u64,
            src: None,
            text: Some(text.clone()),
            font_size: Some(font_size),
            paragraphs: if item.paragraphs.is_empty() {
                None
            } else {
                Some(item.paragraphs)
            },
        });
    }

    if item.kind == "image" && !item.rel_id.is_empty() {
        if let Some(target) = rels.get(&item.rel_id) {
            let path = normalize_pptx_target(target);
            if let Ok(data) = zip_entry_bytes(bytes, &path) {
                let mime = image_mime(&path);
                let encoded = base64::engine::general_purpose::STANDARD.encode(&data);
                let x = item.x.clamp(0, sw) as u64;
                let y = item.y.clamp(0, sh) as u64;
                let w = item.w.clamp(0, sw.saturating_sub(item.x)) as u64;
                let h = item.h.clamp(0, sh.saturating_sub(item.y)) as u64;
                if w == 0 || h == 0 {
                    return None;
                }
                return Some(PptxElement {
                    kind: "image".into(),
                    x,
                    y,
                    w,
                    h,
                    src: Some(format!("data:{mime};base64,{encoded}")),
                    text: None,
                    font_size: None,
                    paragraphs: None,
                });
            }
        }
    }

    None
}

fn element_box(item: &ElementBuilder, text: &str, sw: i64, sh: i64) -> (i64, i64, i64, i64) {
    let x = item.x.clamp(0, sw);
    let y = item.y.clamp(0, sh);
    let w = item.w.clamp(0, sw.saturating_sub(x));
    let h = item.h.clamp(0, sh.saturating_sub(y));
    if w > 0 && h > 0 {
        return (x, y, w, h);
    }
    fallback_text_box(&item.name, text, sw, sh)
}

fn fallback_text_box(name: &str, text: &str, sw: i64, sh: i64) -> (i64, i64, i64, i64) {
    let lower = name.to_ascii_lowercase();
    if lower.contains("title") || text.len() <= 80 {
        (sw * 7 / 100, sh * 8 / 100, sw * 86 / 100, sh * 18 / 100)
    } else {
        (sw * 9 / 100, sh * 30 / 100, sw * 82 / 100, sh * 58 / 100)
    }
}

fn fallback_font_size(name: &str, text: &str) -> f64 {
    let lower = name.to_ascii_lowercase();
    if lower.contains("title") || text.len() <= 80 {
        40.0
    } else {
        24.0
    }
}

fn normalize_pptx_target(target: &str) -> String {
    let clean = target.trim_start_matches('/');
    if clean.starts_with("ppt/") {
        clean.to_string()
    } else if clean.starts_with("../") {
        format!("ppt/{}", clean.trim_start_matches("../"))
    } else {
        format!("ppt/slides/{clean}")
    }
}

fn image_mime(path: &str) -> &'static str {
    match path
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        _ => "image/png",
    }
}

fn x_attr(e: &quick_xml::events::BytesStart<'_>, key: &[u8]) -> Option<String> {
    e.attributes().flatten().find_map(|a| {
        if a.key.as_ref() == key {
            Some(String::from_utf8_lossy(a.value.as_ref()).to_string())
        } else {
            None
        }
    })
}

fn zip_entry_string(bytes: &[u8], name: &str) -> Result<String, Error> {
    let mut archive =
        ZipArchive::new(Cursor::new(bytes)).map_err(|e| Error::Parse(format!("zip open: {e}")))?;
    let mut xml = String::new();
    archive
        .by_name(name)
        .map_err(|e| Error::Parse(format!("{name}: {e}")))?
        .read_to_string(&mut xml)
        .map_err(|e| Error::Parse(format!("{name} read: {e}")))?;
    Ok(xml)
}

fn zip_entry_bytes(bytes: &[u8], name: &str) -> Result<Vec<u8>, Error> {
    let mut archive =
        ZipArchive::new(Cursor::new(bytes)).map_err(|e| Error::Parse(format!("zip open: {e}")))?;
    let mut data = Vec::new();
    archive
        .by_name(name)
        .map_err(|e| Error::Parse(format!("{name}: {e}")))?
        .read_to_end(&mut data)
        .map_err(|e| Error::Parse(format!("{name} read: {e}")))?;
    Ok(data)
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

    #[test]
    fn extract_slide_text_falls_back_without_placeholder_types() {
        let xml = r#"
        <p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
          <p:cSld><p:spTree>
            <p:sp><p:nvSpPr><p:cNvPr id="2"/></p:nvSpPr><p:txBody><a:p><a:r><a:t>Hello Black</a:t></a:r></a:p></p:txBody></p:sp>
            <p:sp><p:nvSpPr><p:cNvPr id="3"/></p:nvSpPr><p:txBody><a:p><a:r><a:t>First bullet</a:t></a:r></a:p><a:p><a:r><a:t>Second bullet</a:t></a:r></a:p></p:txBody></p:sp>
          </p:spTree></p:cSld>
        </p:sld>
        "#;
        let (title, body) = extract_slide_text(xml);
        assert_eq!(title, "Hello Black");
        assert!(body.contains("First bullet"));
        assert!(body.contains("Second bullet"));
    }

    #[test]
    fn parse_slide_elements_extracts_positioned_text() {
        let xml = r#"
        <p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
          <p:bg><p:bgPr><a:solidFill><a:srgbClr val="0F2B46"/></a:solidFill></p:bgPr></p:bg>
          <p:cSld><p:spTree>
            <p:sp>
              <p:nvSpPr><p:cNvPr id="2" name="Title 1"/></p:nvSpPr>
              <p:spPr><a:xfrm><a:off x="457200" y="270000"/><a:ext cx="2000000" cy="500000"/></a:xfrm></p:spPr>
              <p:txBody><a:bodyPr/><a:p><a:pPr><a:buNone/></a:pPr><a:r><a:rPr b="1" sz="3600"/><a:t>Exact deck</a:t></a:r></a:p></p:txBody>
            </p:sp>
          </p:spTree></p:cSld>
        </p:sld>
        "#;
        let bg = parse_slide_background(xml).unwrap();
        assert_eq!(bg.kind, "solid");
        assert_eq!(bg.color.as_deref(), Some("#0F2B46"));

        let rels = std::collections::HashMap::new();
        let els = parse_slide_elements(&[], xml, &rels, DEFAULT_SLIDE_W, DEFAULT_SLIDE_H);
        assert_eq!(els.len(), 1);
        let el = &els[0];
        assert_eq!(el.kind, "text");
        assert_eq!(el.text.as_deref(), Some("Exact deck"));
        assert!(el.x > 0 && el.w > 0);
        let para = el.paragraphs.as_ref().unwrap();
        assert_eq!(para[0].runs[0].text, "Exact deck");
        assert!(para[0].runs[0].bold);
    }
}
