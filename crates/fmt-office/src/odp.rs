//! ODP / OTP parser (OpenDocument Presentation).

use base64::Engine as _;
use std::collections::HashMap;
use std::io::{Cursor, Read};
use viewit_core_types::{
    Document, Error, Format, PptxBackground, PptxElement, PptxParagraph, PptxRun, PptxSlide,
};
use zip::ZipArchive;

const DEFAULT_WIDTH: u64 = 9_144_000;
const DEFAULT_HEIGHT: u64 = 6_858_000;
const MAX_IMAGE_ASSETS: usize = 64;
const MAX_IMAGE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_TOTAL_IMAGE_BYTES: usize = 64 * 1024 * 1024;

pub fn parse_odp(bytes: &[u8], _format: Format, _name: &str) -> Result<Document, Error> {
    let cursor = Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor).map_err(|e| Error::Parse(format!("zip: {}", e)))?;

    let Some(content_xml) = read_zip_string(&mut archive, "content.xml") else {
        return Ok(Document::Text {
            content: "(ODP archive with no content.xml)".into(),
            encoding: "utf-8".into(),
            truncated: false,
            byte_len: bytes.len(),
            stream_url: None,
        });
    };
    let styles_xml = read_zip_string(&mut archive, "styles.xml").unwrap_or_default();
    let assets = read_package_assets(&mut archive);
    let slides = extract_odp_package(&content_xml, &styles_xml, &assets);

    Ok(Document::Pptx {
        slide_count: slides.len(),
        slides,
        byte_len: bytes.len(),
        asset_path: String::new(),
        stream_url: None,
    })
}

fn read_zip_string(archive: &mut ZipArchive<Cursor<&[u8]>>, name: &str) -> Option<String> {
    let mut value = String::new();
    archive
        .by_name(name)
        .ok()?
        .read_to_string(&mut value)
        .ok()?;
    Some(value)
}

fn read_package_assets(archive: &mut ZipArchive<Cursor<&[u8]>>) -> HashMap<String, String> {
    let mut assets = HashMap::new();
    let mut total_bytes = 0usize;
    for index in 0..archive.len() {
        let Ok(mut file) = archive.by_index(index) else {
            continue;
        };
        let name = file.name().trim_start_matches("./").to_string();
        let Some(mime) = image_mime(&name) else {
            continue;
        };
        if assets.len() >= MAX_IMAGE_ASSETS
            || file.size() > MAX_IMAGE_BYTES
            || total_bytes >= MAX_TOTAL_IMAGE_BYTES
        {
            continue;
        }
        let mut data = Vec::new();
        if file
            .by_ref()
            .take(MAX_IMAGE_BYTES + 1)
            .read_to_end(&mut data)
            .is_ok()
            && data.len() as u64 <= MAX_IMAGE_BYTES
            && total_bytes.saturating_add(data.len()) <= MAX_TOTAL_IMAGE_BYTES
        {
            total_bytes += data.len();
            let encoded = base64::engine::general_purpose::STANDARD.encode(data);
            assets.insert(name, format!("data:{mime};base64,{encoded}"));
        }
    }
    assets
}

fn image_mime(path: &str) -> Option<&'static str> {
    match path.rsplit('.').next()?.to_ascii_lowercase().as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "bmp" => Some("image/bmp"),
        "webp" => Some("image/webp"),
        "svg" => Some("image/svg+xml"),
        "tif" | "tiff" => Some("image/tiff"),
        _ => None,
    }
}

#[derive(Clone, Default)]
struct StyleDef {
    parent: Option<String>,
    font_size: Option<f64>,
    bold: Option<bool>,
    italic: Option<bool>,
    underline: Option<bool>,
    color: Option<String>,
    font_family: Option<String>,
    alignment: Option<String>,
    fill_color: Option<String>,
    border_color: Option<String>,
    border_width: Option<f64>,
}

impl StyleDef {
    fn overlay(&mut self, other: &Self) {
        macro_rules! copy_some {
            ($field:ident) => {
                if other.$field.is_some() {
                    self.$field.clone_from(&other.$field);
                }
            };
        }
        copy_some!(font_size);
        copy_some!(bold);
        copy_some!(italic);
        copy_some!(underline);
        copy_some!(color);
        copy_some!(font_family);
        copy_some!(alignment);
        copy_some!(fill_color);
        copy_some!(border_color);
        copy_some!(border_width);
    }
}

#[derive(Default)]
struct OdpStyles {
    styles: HashMap<String, StyleDef>,
    page_layouts: HashMap<String, (u64, u64)>,
    masters: HashMap<String, MasterPage>,
}

#[derive(Default)]
struct MasterPage {
    layout: Option<String>,
    style: Option<String>,
    elements: Vec<PptxElement>,
}

impl OdpStyles {
    fn resolved(&self, name: Option<&str>) -> StyleDef {
        let mut chain = Vec::new();
        let mut current = name;
        for _ in 0..32 {
            let Some(style_name) = current else { break };
            let Some(style) = self.styles.get(style_name) else {
                break;
            };
            chain.push(style);
            current = style.parent.as_deref();
        }
        let mut result = StyleDef::default();
        for style in chain.into_iter().rev() {
            result.overlay(style);
        }
        result
    }
}

fn parse_styles(xml: &str, result: &mut OdpStyles) {
    use quick_xml::events::Event;

    let mut reader = quick_xml::Reader::from_str(xml);
    let mut current_style: Option<(String, StyleDef)> = None;
    let mut current_layout: Option<String> = None;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => match e.name().local_name().as_ref() {
                b"style" => {
                    if let Some(name) = attr(&e, b"name") {
                        current_style = Some((
                            name,
                            StyleDef {
                                parent: attr(&e, b"parent-style-name"),
                                ..Default::default()
                            },
                        ));
                    }
                }
                b"text-properties" => {
                    if let Some((_, style)) = &mut current_style {
                        style.font_size = attr(&e, b"font-size").and_then(|v| odf_point_value(&v));
                        style.bold = attr(&e, b"font-weight").map(|v| v == "bold");
                        style.italic = attr(&e, b"font-style").map(|v| v == "italic");
                        style.underline = attr(&e, b"text-underline-style").map(|v| v != "none");
                        style.color = attr(&e, b"color");
                        style.font_family = attr(&e, b"font-name")
                            .or_else(|| attr(&e, b"font-family"))
                            .map(|v| v.trim_matches('\'').trim_matches('"').to_string());
                    }
                }
                b"paragraph-properties" => {
                    if let Some((_, style)) = &mut current_style {
                        style.alignment = attr(&e, b"text-align").map(normalize_alignment);
                    }
                }
                b"graphic-properties" | b"drawing-page-properties" => {
                    if let Some((_, style)) = &mut current_style {
                        apply_graphic_properties(style, &e);
                    }
                }
                b"page-layout" => current_layout = attr(&e, b"name"),
                b"page-layout-properties" => {
                    if let Some(name) = &current_layout {
                        if let (Some(width), Some(height)) = (
                            odf_length_attr(&e, b"page-width"),
                            odf_length_attr(&e, b"page-height"),
                        ) {
                            result.page_layouts.insert(name.clone(), (width, height));
                        }
                    }
                }
                b"master-page" => {
                    if let Some(name) = attr(&e, b"name") {
                        result.masters.insert(
                            name,
                            MasterPage {
                                layout: attr(&e, b"page-layout-name"),
                                style: attr(&e, b"style-name"),
                                elements: Vec::new(),
                            },
                        );
                    }
                }
                _ => {}
            },
            Ok(Event::End(e)) => match e.name().local_name().as_ref() {
                b"style" => {
                    if let Some((name, style)) = current_style.take() {
                        result.styles.insert(name, style);
                    }
                }
                b"page-layout" => current_layout = None,
                _ => {}
            },
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

fn apply_graphic_properties(style: &mut StyleDef, e: &quick_xml::events::BytesStart<'_>) {
    if attr(e, b"fill").as_deref() != Some("none") {
        style.fill_color = attr(e, b"fill-color");
    }
    if attr(e, b"stroke").as_deref() != Some("none") {
        style.border_color = attr(e, b"stroke-color");
        style.border_width = attr(e, b"stroke-width").and_then(|v| odf_point_value(&v));
    }
}

#[cfg(test)]
fn extract_odp_slides(xml: &str) -> Vec<PptxSlide> {
    extract_odp_package(xml, "", &HashMap::new())
}

fn extract_odp_package(
    content_xml: &str,
    styles_xml: &str,
    assets: &HashMap<String, String>,
) -> Vec<PptxSlide> {
    let mut styles = OdpStyles::default();
    parse_styles(styles_xml, &mut styles);
    parse_styles(content_xml, &mut styles);
    for xml in [styles_xml, content_xml] {
        for master in parse_containers(xml, &styles, assets, true) {
            let Some(name) = master.name.as_deref() else {
                continue;
            };
            if let Some(definition) = styles.masters.get_mut(name) {
                merge_elements(&mut definition.elements, master.elements);
            }
        }
    }
    parse_pages(content_xml, &styles, assets)
}

#[derive(Default)]
struct PageBuilder {
    name: Option<String>,
    master: Option<String>,
    style: Option<String>,
    texts: Vec<String>,
    elements: Vec<PptxElement>,
}

struct ElementBuilder {
    kind: &'static str,
    x: u64,
    y: u64,
    w: u64,
    h: u64,
    style: Option<String>,
    href: Option<String>,
    paragraphs: Vec<PptxParagraph>,
}

struct ParagraphBuilder {
    style: Option<String>,
    runs: Vec<PptxRun>,
}

fn parse_pages(xml: &str, styles: &OdpStyles, assets: &HashMap<String, String>) -> Vec<PptxSlide> {
    parse_containers(xml, styles, assets, false)
        .into_iter()
        .map(|page| finish_page(page, styles))
        .collect()
}

fn parse_containers(
    xml: &str,
    styles: &OdpStyles,
    assets: &HashMap<String, String>,
    masters: bool,
) -> Vec<PageBuilder> {
    use quick_xml::events::Event;

    let mut reader = quick_xml::Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut pages = Vec::new();
    let mut page: Option<PageBuilder> = None;
    let mut element: Option<ElementBuilder> = None;
    let mut paragraph: Option<ParagraphBuilder> = None;
    let mut spans: Vec<Option<String>> = Vec::new();
    let mut notes_depth = 0usize;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let tag = e.name().local_name();
                match tag.as_ref() {
                    b"page" if !masters && page.is_none() => {
                        page = Some(PageBuilder {
                            master: attr(&e, b"master-page-name"),
                            style: attr(&e, b"style-name"),
                            ..Default::default()
                        });
                    }
                    b"master-page" if masters && page.is_none() => {
                        page = Some(PageBuilder {
                            name: attr(&e, b"name"),
                            ..Default::default()
                        });
                    }
                    b"notes" if page.is_some() => notes_depth += 1,
                    b"frame" if page.is_some() && notes_depth == 0 => {
                        finish_element(&mut page, &mut element, styles, assets);
                        element = Some(new_element("text", &e));
                    }
                    b"rect" | b"custom-shape" if page.is_some() && notes_depth == 0 => {
                        finish_element(&mut page, &mut element, styles, assets);
                        element = Some(new_element("shape", &e));
                    }
                    b"image" if element.is_some() && notes_depth == 0 => {
                        let item = element.as_mut().unwrap();
                        item.kind = "image";
                        item.href = attr(&e, b"href");
                    }
                    b"p" if page.is_some() && notes_depth == 0 => {
                        paragraph = Some(ParagraphBuilder {
                            style: attr(&e, b"style-name"),
                            runs: Vec::new(),
                        });
                    }
                    b"span" if paragraph.is_some() => spans.push(attr(&e, b"style-name")),
                    b"tab" if paragraph.is_some() => append_text(
                        paragraph.as_mut().unwrap(),
                        spans.last().and_then(|v| v.as_deref()),
                        "\t",
                        styles,
                    ),
                    b"line-break" if paragraph.is_some() => append_text(
                        paragraph.as_mut().unwrap(),
                        spans.last().and_then(|v| v.as_deref()),
                        "\n",
                        styles,
                    ),
                    _ => {}
                }
            }
            Ok(Event::Empty(e)) => match e.name().local_name().as_ref() {
                b"rect" | b"custom-shape" if page.is_some() && notes_depth == 0 => {
                    finish_element(&mut page, &mut element, styles, assets);
                    element = Some(new_element("shape", &e));
                    finish_element(&mut page, &mut element, styles, assets);
                }
                b"image" if element.is_some() && notes_depth == 0 => {
                    let item = element.as_mut().unwrap();
                    item.kind = "image";
                    item.href = attr(&e, b"href");
                }
                b"tab" if paragraph.is_some() => append_text(
                    paragraph.as_mut().unwrap(),
                    spans.last().and_then(|v| v.as_deref()),
                    "\t",
                    styles,
                ),
                b"line-break" if paragraph.is_some() => append_text(
                    paragraph.as_mut().unwrap(),
                    spans.last().and_then(|v| v.as_deref()),
                    "\n",
                    styles,
                ),
                _ => {}
            },
            Ok(Event::Text(t)) if paragraph.is_some() && notes_depth == 0 => {
                let text = t.unescape().map(|v| v.into_owned()).unwrap_or_default();
                append_text(
                    paragraph.as_mut().unwrap(),
                    spans.last().and_then(|v| v.as_deref()),
                    &text,
                    styles,
                );
            }
            Ok(Event::End(e)) => match e.name().local_name().as_ref() {
                b"span" if paragraph.is_some() => {
                    spans.pop();
                }
                b"p" if paragraph.is_some() => {
                    let built = finish_paragraph(paragraph.take().unwrap(), styles);
                    let text: String = built.runs.iter().map(|run| run.text.as_str()).collect();
                    if !text.trim().is_empty() {
                        if let Some(page) = &mut page {
                            page.texts.push(text);
                        }
                        if let Some(item) = &mut element {
                            item.paragraphs.push(built);
                        }
                    }
                    spans.clear();
                }
                b"frame" | b"rect" | b"custom-shape" if notes_depth == 0 => {
                    finish_element(&mut page, &mut element, styles, assets);
                }
                b"notes" if notes_depth > 0 => notes_depth -= 1,
                b"page" if !masters && page.is_some() && notes_depth == 0 => {
                    finish_element(&mut page, &mut element, styles, assets);
                    pages.push(page.take().unwrap());
                }
                b"master-page" if masters && page.is_some() && notes_depth == 0 => {
                    finish_element(&mut page, &mut element, styles, assets);
                    pages.push(page.take().unwrap());
                }
                _ => {}
            },
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    pages
}

fn new_element(kind: &'static str, e: &quick_xml::events::BytesStart<'_>) -> ElementBuilder {
    ElementBuilder {
        kind,
        x: odf_length_attr(e, b"x").unwrap_or(0),
        y: odf_length_attr(e, b"y").unwrap_or(0),
        w: odf_length_attr(e, b"width").unwrap_or(0),
        h: odf_length_attr(e, b"height").unwrap_or(0),
        style: attr(e, b"style-name"),
        href: None,
        paragraphs: Vec::new(),
    }
}

fn append_text(
    paragraph: &mut ParagraphBuilder,
    span: Option<&str>,
    text: &str,
    styles: &OdpStyles,
) {
    if text.is_empty() {
        return;
    }
    let mut style = styles.resolved(paragraph.style.as_deref());
    style.overlay(&styles.resolved(span));
    let run = PptxRun {
        text: text.to_string(),
        bold: style.bold.unwrap_or(false),
        italic: style.italic.unwrap_or(false),
        underline: style.underline.unwrap_or(false),
        font_size: style.font_size,
        color: style.color,
        font_family: style.font_family,
    };
    if let Some(previous) = paragraph.runs.last_mut() {
        if previous.bold == run.bold
            && previous.italic == run.italic
            && previous.underline == run.underline
            && previous.font_size == run.font_size
            && previous.color == run.color
            && previous.font_family == run.font_family
        {
            previous.text.push_str(text);
            return;
        }
    }
    paragraph.runs.push(run);
}

fn finish_paragraph(paragraph: ParagraphBuilder, styles: &OdpStyles) -> PptxParagraph {
    PptxParagraph {
        runs: paragraph.runs,
        bullet: false,
        alignment: styles.resolved(paragraph.style.as_deref()).alignment,
    }
}

fn finish_element(
    page: &mut Option<PageBuilder>,
    element: &mut Option<ElementBuilder>,
    styles: &OdpStyles,
    assets: &HashMap<String, String>,
) {
    let Some(item) = element.take() else { return };
    let Some(page) = page else { return };
    let text = item
        .paragraphs
        .iter()
        .map(|p| p.runs.iter().map(|r| r.text.as_str()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    if item.kind == "text" && text.is_empty() {
        return;
    }
    let style = styles.resolved(item.style.as_deref());
    let index = page.elements.len();
    let (x, y, w, h) = if item.w > 0 && item.h > 0 {
        (item.x, item.y, item.w, item.h)
    } else if index == 0 {
        (640_080, 548_640, 7_863_840, 1_234_440)
    } else {
        (822_960, 2_057_400, 7_498_080, 3_977_640)
    };
    let src = item.href.as_deref().map(|href| {
        let normalized = href.trim_start_matches("./");
        assets.get(normalized).cloned().unwrap_or_default()
    });
    let font_size = item
        .paragraphs
        .iter()
        .flat_map(|p| &p.runs)
        .find_map(|run| run.font_size)
        .or(Some(if index == 0 { 36.0 } else { 24.0 }));
    page.elements.push(PptxElement {
        kind: if item.kind == "shape" && !text.is_empty() { "text" } else { item.kind }.into(),
        x,
        y,
        w,
        h,
        src,
        text: (!text.is_empty()).then_some(text),
        font_size,
        paragraphs: (!item.paragraphs.is_empty()).then_some(item.paragraphs),
        fill_color: style.fill_color,
        border_color: style.border_color,
        border_width: style.border_width,
    });
}

fn finish_page(page: PageBuilder, styles: &OdpStyles) -> PptxSlide {
    let master = page
        .master
        .as_deref()
        .and_then(|name| styles.masters.get(name));
    let (width, height) = master
        .and_then(|master| master.layout.as_deref())
        .and_then(|name| styles.page_layouts.get(name))
        .copied()
        .unwrap_or((DEFAULT_WIDTH, DEFAULT_HEIGHT));
    let mut background_style = styles.resolved(master.and_then(|master| master.style.as_deref()));
    background_style.overlay(&styles.resolved(page.style.as_deref()));
    let background = background_style.fill_color.map(|color| PptxBackground {
        kind: "solid".into(),
        color: Some(color),
        gradient: None,
    });
    let mut elements = Vec::new();
    if let Some(master) = master {
        merge_elements(&mut elements, master.elements.clone());
    }
    merge_elements(&mut elements, page.elements);
    PptxSlide {
        title: page.texts.first().cloned().unwrap_or_default(),
        body: page.texts.get(1..).unwrap_or_default().join("\n"),
        elements,
        width: Some(width),
        height: Some(height),
        background,
    }
}

fn merge_elements(target: &mut Vec<PptxElement>, source: Vec<PptxElement>) {
    let inherited_count = target.len();
    for element in source {
        if !target
            .get(..inherited_count)
            .unwrap_or_default()
            .iter()
            .any(|existing| same_element(existing, &element))
        {
            target.push(element);
        }
    }
}

fn same_element(left: &PptxElement, right: &PptxElement) -> bool {
    left.kind == right.kind
        && left.x == right.x
        && left.y == right.y
        && left.w == right.w
        && left.h == right.h
        && left.src == right.src
        && left.text == right.text
        && (left.kind != "text" || left.font_size == right.font_size)
        && same_paragraphs(left.paragraphs.as_deref(), right.paragraphs.as_deref())
        && left.fill_color == right.fill_color
        && left.border_color == right.border_color
        && left.border_width == right.border_width
}

fn same_paragraphs(left: Option<&[PptxParagraph]>, right: Option<&[PptxParagraph]>) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) if left.len() == right.len() => {
            left.iter().zip(right).all(|(left, right)| {
                left.bullet == right.bullet
                    && left.alignment == right.alignment
                    && left.runs.len() == right.runs.len()
                    && left.runs.iter().zip(&right.runs).all(|(left, right)| {
                        left.text == right.text
                            && left.bold == right.bold
                            && left.italic == right.italic
                            && left.underline == right.underline
                            && left.font_size == right.font_size
                            && left.color == right.color
                            && left.font_family == right.font_family
                    })
            })
        }
        _ => false,
    }
}

fn attr(e: &quick_xml::events::BytesStart<'_>, local: &[u8]) -> Option<String> {
    e.attributes().flatten().find_map(|attribute| {
        (attribute.key.local_name().as_ref() == local)
            .then(|| String::from_utf8_lossy(attribute.value.as_ref()).into_owned())
    })
}

fn odf_length_attr(e: &quick_xml::events::BytesStart<'_>, local: &[u8]) -> Option<u64> {
    attr(e, local).and_then(|value| odf_length_to_emu(&value))
}

fn odf_length_to_emu(value: &str) -> Option<u64> {
    let split = value.find(|c: char| !c.is_ascii_digit() && c != '.' && c != '-')?;
    let amount = value[..split].parse::<f64>().ok()?;
    let emu = match &value[split..] {
        "cm" => amount * 360_000.0,
        "mm" => amount * 36_000.0,
        "in" => amount * 914_400.0,
        "pt" => amount * 12_700.0,
        "pc" => amount * 152_400.0,
        _ => return None,
    };
    Some(emu.max(0.0).round() as u64)
}

fn odf_point_value(value: &str) -> Option<f64> {
    let split = value.find(|c: char| !c.is_ascii_digit() && c != '.' && c != '-')?;
    let amount = value[..split].parse::<f64>().ok()?;
    match &value[split..] {
        "pt" => Some(amount),
        "pc" => Some(amount * 12.0),
        "in" => Some(amount * 72.0),
        "cm" => Some(amount * 72.0 / 2.54),
        "mm" => Some(amount * 72.0 / 25.4),
        _ => None,
    }
}

fn normalize_alignment(value: String) -> String {
    match value.as_str() {
        "center" => "ctr".into(),
        "end" | "right" => "r".into(),
        "justify" => "just".into(),
        _ => "l".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    #[test]
    fn extracts_odp_slides_with_fallbacks() {
        let xml = r#"<office:document-content xmlns:office="o" xmlns:draw="d" xmlns:text="t"><office:body><office:presentation><draw:page><text:p>Slide Title</text:p><text:p>Body one</text:p><text:p>Body two</text:p></draw:page></office:presentation></office:body></office:document-content>"#;
        let slides = extract_odp_slides(xml);
        assert_eq!(slides.len(), 1);
        assert_eq!(slides[0].title, "Slide Title");
        assert_eq!(slides[0].body, "Body one\nBody two");
        assert_eq!(
            (slides[0].width, slides[0].height),
            (Some(DEFAULT_WIDTH), Some(DEFAULT_HEIGHT))
        );
    }

    #[test]
    fn extracts_independent_positioned_frames() {
        let xml = r#"<office:document-content xmlns:office="o" xmlns:draw="d" xmlns:text="t" xmlns:svg="s"><office:body><office:presentation><draw:page><draw:frame svg:x="1cm" svg:y="2cm" svg:width="10cm" svg:height="3cm"><draw:text-box><text:p>One</text:p></draw:text-box></draw:frame><draw:frame svg:x="2cm" svg:y="3cm" svg:width="4cm" svg:height="5cm"><draw:text-box><text:p>Two</text:p></draw:text-box></draw:frame></draw:page></office:presentation></office:body></office:document-content>"#;
        let slide = &extract_odp_slides(xml)[0];
        assert_eq!(slide.elements.len(), 2);
        assert_eq!(
            (slide.elements[0].x, slide.elements[0].y),
            (360_000, 720_000)
        );
        assert_eq!(slide.elements[0].text.as_deref(), Some("One"));
        assert_eq!(slide.elements[1].text.as_deref(), Some("Two"));
    }

    #[test]
    fn text_bearing_custom_shape_renders_as_text_element() {
        let xml = r#"<office:document-content xmlns:office="o" xmlns:draw="d" xmlns:text="t" xmlns:svg="s"><office:body><office:presentation><draw:page><draw:custom-shape svg:x="1cm" svg:y="2cm" svg:width="3cm" svg:height="1cm"><text:p>Visible title</text:p></draw:custom-shape></draw:page></office:presentation></office:body></office:document-content>"#;
        let slide = &extract_odp_slides(xml)[0];
        assert_eq!(slide.elements[0].kind, "text");
        assert_eq!(slide.elements[0].text.as_deref(), Some("Visible title"));
    }

    #[test]
    fn resolves_layout_styles_shapes_and_background() {
        let styles_xml = r##"<office:document-styles xmlns:office="o" xmlns:style="s" xmlns:fo="f" xmlns:draw="d" xmlns:svg="v"><office:styles><style:style style:name="page-bg" style:family="drawing-page"><style:drawing-page-properties draw:fill="solid" draw:fill-color="#112233"/></style:style></office:styles><office:automatic-styles><style:page-layout style:name="wide"><style:page-layout-properties fo:page-width="30cm" fo:page-height="20cm"/></style:page-layout><style:style style:name="shape" style:family="graphic"><style:graphic-properties draw:fill="solid" draw:fill-color="#abcdef" draw:stroke="solid" svg:stroke-color="#010203" svg:stroke-width="2pt"/></style:style></office:automatic-styles><office:master-styles><style:master-page style:name="Master" style:page-layout-name="wide" draw:style-name="page-bg"/></office:master-styles></office:document-styles>"##;
        let content = r#"<office:document-content xmlns:office="o" xmlns:draw="d" xmlns:svg="v"><office:body><office:presentation><draw:page draw:master-page-name="Master"><draw:rect draw:style-name="shape" svg:x="1cm" svg:y="1cm" svg:width="2cm" svg:height="3cm"/></draw:page></office:presentation></office:body></office:document-content>"#;
        let slide = &extract_odp_package(content, styles_xml, &HashMap::new())[0];
        assert_eq!(
            (slide.width, slide.height),
            (Some(10_800_000), Some(7_200_000))
        );
        assert_eq!(
            slide.background.as_ref().and_then(|b| b.color.as_deref()),
            Some("#112233")
        );
        assert_eq!(slide.elements[0].kind, "shape");
        assert_eq!(slide.elements[0].fill_color.as_deref(), Some("#abcdef"));
        assert_eq!(slide.elements[0].border_width, Some(2.0));
    }

    #[test]
    fn inherits_visible_master_objects_without_duplicates() {
        let styles_xml = r##"<office:document-styles xmlns:office="o" xmlns:style="s" xmlns:draw="d" xmlns:text="t" xmlns:fo="f" xmlns:svg="v" xmlns:xlink="x"><office:styles><style:style style:name="master-text" style:family="paragraph"><style:text-properties fo:font-size="18pt" fo:font-weight="bold" fo:color="#123456"/></style:style><style:style style:name="master-shape" style:family="graphic"><style:graphic-properties draw:fill="solid" draw:fill-color="#abcdef"/></style:style></office:styles><office:automatic-styles><style:page-layout style:name="wide"><style:page-layout-properties fo:page-width="30cm" fo:page-height="20cm"/></style:page-layout></office:automatic-styles><office:master-styles><style:master-page style:name="Master" style:page-layout-name="wide"><draw:frame svg:x="1cm" svg:y="1cm" svg:width="4cm" svg:height="1cm"><draw:text-box><text:p text:style-name="master-text">Inherited</text:p></draw:text-box></draw:frame><draw:frame svg:x="2cm" svg:y="3cm" svg:width="2cm" svg:height="2cm"><draw:image xlink:href="Pictures/logo.png"/></draw:frame><draw:rect draw:style-name="master-shape" svg:x="3cm" svg:y="4cm" svg:width="5cm" svg:height="2cm"/><draw:custom-shape draw:style-name="master-shape" svg:x="4cm" svg:y="5cm" svg:width="1cm" svg:height="1cm"/></style:master-page></office:master-styles></office:document-styles>"##;
        let content = r#"<office:document-content xmlns:office="o" xmlns:draw="d" xmlns:svg="v"><office:body><office:presentation><draw:page draw:master-page-name="Master"><draw:rect draw:style-name="master-shape" svg:x="3cm" svg:y="4cm" svg:width="5cm" svg:height="2cm"/><draw:rect svg:x="6cm" svg:y="6cm" svg:width="1cm" svg:height="1cm"/></draw:page></office:presentation></office:body></office:document-content>"#;
        let assets = HashMap::from([(
            "Pictures/logo.png".to_string(),
            "data:image/png;base64,bG9nbw==".to_string(),
        )]);

        let slide = &extract_odp_package(content, styles_xml, &assets)[0];

        assert_eq!(slide.elements.len(), 5);
        assert_eq!(slide.elements[0].text.as_deref(), Some("Inherited"));
        let run = &slide.elements[0].paragraphs.as_ref().unwrap()[0].runs[0];
        assert!(run.bold);
        assert_eq!(run.font_size, Some(18.0));
        assert_eq!(run.color.as_deref(), Some("#123456"));
        assert_eq!(slide.elements[1].kind, "image");
        assert_eq!(
            slide.elements[1].src.as_deref(),
            Some("data:image/png;base64,bG9nbw==")
        );
        assert_eq!(slide.elements[2].fill_color.as_deref(), Some("#abcdef"));
        assert_eq!(
            (slide.elements[2].x, slide.elements[2].y),
            (1_080_000, 1_440_000)
        );
        assert_eq!(
            (slide.elements[3].x, slide.elements[3].y),
            (1_440_000, 1_800_000)
        );
        assert_eq!(
            (slide.elements[4].x, slide.elements[4].y),
            (2_160_000, 2_160_000)
        );
    }

    #[test]
    fn applies_automatic_text_styles() {
        let xml = r##"<office:document-content xmlns:office="o" xmlns:draw="d" xmlns:text="t" xmlns:style="s" xmlns:fo="f"><office:automatic-styles><style:style style:name="para" style:family="paragraph"><style:paragraph-properties fo:text-align="center"/><style:text-properties fo:font-size="20pt" fo:font-weight="bold" style:font-name="Inter"/></style:style><style:style style:name="em" style:family="text"><style:text-properties fo:font-style="italic" style:text-underline-style="solid" fo:color="#aabbcc"/></style:style></office:automatic-styles><office:body><office:presentation><draw:page><draw:frame><draw:text-box><text:p text:style-name="para">Normal <text:span text:style-name="em">styled</text:span></text:p></draw:text-box></draw:frame></draw:page></office:presentation></office:body></office:document-content>"##;
        let slide = &extract_odp_slides(xml)[0];
        let paragraph = &slide.elements[0].paragraphs.as_ref().unwrap()[0];
        assert_eq!(paragraph.alignment.as_deref(), Some("ctr"));
        assert!(paragraph.runs[0].bold);
        assert_eq!(paragraph.runs[0].font_size, Some(20.0));
        assert_eq!(paragraph.runs[0].font_family.as_deref(), Some("Inter"));
        assert!(paragraph.runs[1].bold && paragraph.runs[1].italic && paragraph.runs[1].underline);
        assert_eq!(paragraph.runs[1].color.as_deref(), Some("#aabbcc"));
    }

    #[test]
    fn parse_package_embeds_image_data_uri() {
        let content = r#"<office:document-content xmlns:office="o" xmlns:draw="d" xmlns:xlink="x" xmlns:svg="s"><office:body><office:presentation><draw:page><draw:frame svg:width="1cm" svg:height="1cm"><draw:image xlink:href="Pictures/pixel.png"/></draw:frame></draw:page></office:presentation></office:body></office:document-content>"#;
        let mut bytes = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut bytes);
            zip.start_file("content.xml", SimpleFileOptions::default())
                .unwrap();
            zip.write_all(content.as_bytes()).unwrap();
            zip.start_file("styles.xml", SimpleFileOptions::default())
                .unwrap();
            zip.write_all(b"<office:document-styles xmlns:office=\"o\"/>")
                .unwrap();
            zip.start_file("Pictures/pixel.png", SimpleFileOptions::default())
                .unwrap();
            zip.write_all(b"png").unwrap();
            zip.finish().unwrap();
        }
        let Document::Pptx { slides, .. } =
            parse_odp(bytes.get_ref(), Format::Odp, "image.odp").unwrap()
        else {
            panic!()
        };
        assert_eq!(slides[0].elements[0].kind, "image");
        assert_eq!(
            slides[0].elements[0].src.as_deref(),
            Some("data:image/png;base64,cG5n")
        );
    }
}
