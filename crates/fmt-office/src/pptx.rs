//! PPTX / PPTM / POTX parser — unified for office-universal.
//!
//! Produces `Document::Pptx` with a real slide viewport: slide extents + solid
//! background, and positioned elements (text boxes with styled runs, images)
//! so viewers render the deck's actual layout instead of a flat text dump.
//! Rich-parse logic mirrors the `office-ooxml` catalog plugin.

use base64::Engine as _;
use std::collections::{HashMap, HashSet};
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
        let inheritance = load_slide_inheritance(bytes, i, &slide_xml, &rels);
        let elements = parse_slide_elements(
            bytes,
            &slide_path,
            &slide_xml,
            &rels,
            slide_w,
            slide_h,
            &inheritance,
        );
        let background = parse_slide_background_with_theme(&slide_xml, &inheritance.theme)
            .or_else(|| inherited_slide_background(&inheritance));

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

#[cfg(test)]
fn parse_slide_background(xml: &str) -> Option<PptxBackground> {
    parse_slide_background_with_theme(xml, &ThemeContext::default())
}

fn parse_slide_background_with_theme(xml: &str, theme: &ThemeContext) -> Option<PptxBackground> {
    use quick_xml::events::Event;

    let mut reader = quick_xml::Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut in_bg = false;
    let mut in_gradient = false;
    let mut in_stop = false;
    let mut stop_pos = 0.0;
    let mut stops: Vec<(f64, String)> = Vec::new();
    let mut angle = 90.0;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if e.name().as_ref() == b"p:bg" => in_bg = true,
            Ok(Event::End(e)) if e.name().as_ref() == b"p:bg" => in_bg = false,
            Ok(Event::Start(e)) if in_bg && e.name().as_ref() == b"a:gradFill" => {
                in_gradient = true
            }
            Ok(Event::End(e)) if e.name().as_ref() == b"a:gradFill" => in_gradient = false,
            Ok(Event::Start(e)) if in_gradient && e.name().as_ref() == b"a:gs" => {
                in_stop = true;
                stop_pos = x_attr(&e, b"pos")
                    .and_then(|value| value.parse::<f64>().ok())
                    .unwrap_or(0.0)
                    / 1000.0;
            }
            Ok(Event::End(e)) if e.name().as_ref() == b"a:gs" => in_stop = false,
            Ok(Event::Empty(e)) if in_gradient && e.name().as_ref() == b"a:lin" => {
                if let Some(value) = x_attr(&e, b"ang").and_then(|value| value.parse::<f64>().ok())
                {
                    angle = (90.0 + value / 60000.0).rem_euclid(360.0);
                }
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if in_bg
                    && (e.name().as_ref() == b"a:srgbClr"
                        || e.name().as_ref() == b"a:schemeClr"
                        || e.name().as_ref() == b"a:sysClr") =>
            {
                if let Some(color) = resolve_color_element(&e, theme) {
                    if in_gradient && in_stop {
                        stops.push((stop_pos, color));
                    } else {
                        return Some(PptxBackground {
                            kind: "solid".into(),
                            color: Some(color),
                            gradient: None,
                        });
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    if stops.len() >= 2 {
        let colors = stops
            .into_iter()
            .map(|(position, color)| format!("{color} {}%", format_number(position)))
            .collect::<Vec<_>>()
            .join(", ");
        return Some(PptxBackground {
            kind: "gradient".into(),
            color: None,
            gradient: Some(format!(
                "linear-gradient({}deg, {colors})",
                format_number(angle)
            )),
        });
    }
    None
}

fn inherited_slide_background(inheritance: &SlideInheritance) -> Option<PptxBackground> {
    if let Some(background) =
        parse_slide_background_with_theme(&inheritance.layout_xml, &inheritance.theme)
    {
        return Some(background);
    }
    if let Some(background) =
        parse_slide_background_with_theme(&inheritance.master_xml, &inheritance.theme)
    {
        return Some(background);
    }
    let scheme = parse_background_scheme(&inheritance.master_xml)?;
    let color = inheritance.theme.resolve_scheme(&scheme)?;
    Some(PptxBackground {
        kind: "solid".into(),
        color: Some(color),
        gradient: None,
    })
}

fn part_relationships(
    bytes: &[u8],
    part_path: &str,
) -> Result<std::collections::HashMap<String, String>, Error> {
    let (dir, name) = part_path.rsplit_once('/').unwrap_or(("", part_path));
    let rels_path = format!("{dir}/_rels/{name}.rels");
    let xml = zip_entry_string(bytes, &rels_path)?;
    let mut rels = std::collections::HashMap::new();
    let mut reader = quick_xml::Reader::from_str(&xml);
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
            Err(e) => return Err(Error::Parse(format!("relationships parse: {e}"))),
            _ => {}
        }
    }
    Ok(rels)
}

fn resolve_part_target(part_path: &str, target: &str) -> String {
    if target.starts_with('/') {
        return target.trim_start_matches('/').to_string();
    }
    let mut parts: Vec<&str> = part_path.split('/').collect();
    parts.pop();
    for component in target.split('/') {
        match component {
            ".." => {
                parts.pop();
            }
            "." | "" => {}
            value => parts.push(value),
        }
    }
    parts.join("/")
}

fn parse_background_scheme(xml: &str) -> Option<String> {
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut in_bg = false;
    loop {
        match reader.read_event() {
            Ok(quick_xml::events::Event::Start(e)) if e.name().as_ref() == b"p:bg" => in_bg = true,
            Ok(quick_xml::events::Event::Empty(e))
                if in_bg && e.name().as_ref() == b"a:schemeClr" =>
            {
                return x_attr(&e, b"val");
            }
            Ok(quick_xml::events::Event::End(e)) if e.name().as_ref() == b"p:bg" => break,
            Ok(quick_xml::events::Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    None
}

#[cfg(test)]
fn parse_color_map(xml: &str, key: &str) -> Option<String> {
    let mut reader = quick_xml::Reader::from_str(xml);
    loop {
        match reader.read_event() {
            Ok(quick_xml::events::Event::Start(e)) | Ok(quick_xml::events::Event::Empty(e))
                if e.name().as_ref() == b"p:clrMap" =>
            {
                return x_attr(&e, key.as_bytes());
            }
            Ok(quick_xml::events::Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    None
}

fn parse_theme_color(xml: &str, key: &str) -> Option<String> {
    let mut reader = quick_xml::Reader::from_str(xml);
    let expected = format!("a:{key}");
    let mut in_color = false;
    loop {
        match reader.read_event() {
            Ok(quick_xml::events::Event::Start(e)) if e.name().as_ref() == expected.as_bytes() => {
                in_color = true
            }
            Ok(quick_xml::events::Event::Empty(e)) if in_color => {
                if e.name().as_ref() == b"a:srgbClr" {
                    return x_attr(&e, b"val").map(|v| format!("#{v}"));
                }
                if e.name().as_ref() == b"a:sysClr" {
                    return x_attr(&e, b"lastClr").map(|v| format!("#{v}"));
                }
            }
            Ok(quick_xml::events::Event::End(e)) if e.name().as_ref() == expected.as_bytes() => {
                break
            }
            Ok(quick_xml::events::Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    None
}

#[derive(Clone, Default)]
struct ThemeContext {
    colors: HashMap<String, String>,
    color_map: HashMap<String, String>,
    major_font: Option<String>,
    minor_font: Option<String>,
}

impl ThemeContext {
    fn resolve_scheme(&self, key: &str) -> Option<String> {
        let mapped = self.color_map.get(key).map(String::as_str).unwrap_or(key);
        self.colors.get(mapped).cloned()
    }

    fn resolve_font(&self, typeface: &str) -> Option<String> {
        match typeface {
            "+mj-lt" | "+mj-ea" | "+mj-cs" => self.major_font.clone(),
            "+mn-lt" | "+mn-ea" | "+mn-cs" => self.minor_font.clone(),
            "" => None,
            value => Some(value.to_string()),
        }
    }
}

#[derive(Clone, Default)]
struct TextDefaults {
    font_size: Option<f64>,
    color: Option<String>,
    font_family: Option<String>,
    alignment: Option<String>,
}

#[derive(Clone, Default)]
struct PlaceholderDefaults {
    x: i64,
    y: i64,
    w: i64,
    h: i64,
    has_geometry: bool,
    text: [TextDefaults; 9],
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct PlaceholderKey {
    kind: String,
    index: Option<String>,
}

#[derive(Default)]
struct SlideInheritance {
    layout_path: String,
    layout_xml: String,
    layout_rels: HashMap<String, String>,
    master_path: String,
    master_xml: String,
    master_rels: HashMap<String, String>,
    theme: ThemeContext,
    master_text_defaults: HashMap<PlaceholderKey, PlaceholderDefaults>,
    layout_text_defaults: HashMap<PlaceholderKey, PlaceholderDefaults>,
    layout_placeholders: HashMap<PlaceholderKey, PlaceholderDefaults>,
    master_placeholders: HashMap<PlaceholderKey, PlaceholderDefaults>,
}

impl SlideInheritance {
    fn placeholder(&self, key: &PlaceholderKey) -> PlaceholderDefaults {
        let mut result = find_placeholder(&self.master_text_defaults, key).unwrap_or_default();
        if let Some(master) = find_placeholder(&self.master_placeholders, key) {
            merge_placeholder(&mut result, &master);
        }
        if let Some(layout) = find_placeholder(&self.layout_text_defaults, key) {
            merge_placeholder(&mut result, &layout);
        }
        if let Some(layout) = find_placeholder(&self.layout_placeholders, key) {
            merge_placeholder(&mut result, &layout);
        }
        result
    }
}

fn load_slide_inheritance(
    bytes: &[u8],
    slide_index: usize,
    slide_xml: &str,
    slide_rels: &HashMap<String, String>,
) -> SlideInheritance {
    let mut result = SlideInheritance::default();
    let Some(layout_target) = slide_rels
        .values()
        .find(|target| target.contains("slideLayout"))
    else {
        return result;
    };
    let layout_path =
        resolve_part_target(&format!("ppt/slides/slide{slide_index}.xml"), layout_target);
    result.layout_path.clone_from(&layout_path);
    result.layout_xml = zip_entry_string(bytes, &layout_path).unwrap_or_default();
    let layout_rels = part_relationships(bytes, &layout_path).unwrap_or_default();
    result.layout_rels.clone_from(&layout_rels);
    let Some(master_target) = layout_rels
        .values()
        .find(|target| target.contains("slideMaster"))
    else {
        return result;
    };
    let master_path = resolve_part_target(&layout_path, master_target);
    result.master_path.clone_from(&master_path);
    result.master_xml = zip_entry_string(bytes, &master_path).unwrap_or_default();
    let master_rels = part_relationships(bytes, &master_path).unwrap_or_default();
    result.master_rels.clone_from(&master_rels);
    if let Some(theme_target) = master_rels.values().find(|target| target.contains("theme")) {
        let theme_path = resolve_part_target(&master_path, theme_target);
        if let Ok(theme_xml) = zip_entry_string(bytes, &theme_path) {
            result.theme = parse_theme(&theme_xml);
        }
    }
    apply_color_map(&result.master_xml, &mut result.theme.color_map);
    apply_color_map(&result.layout_xml, &mut result.theme.color_map);
    apply_color_map(slide_xml, &mut result.theme.color_map);
    result.master_placeholders = parse_placeholder_defaults(&result.master_xml, &result.theme);
    result.master_text_defaults = parse_text_style_defaults(&result.master_xml, &result.theme);
    result.layout_text_defaults = parse_text_style_defaults(&result.layout_xml, &result.theme);
    result.layout_placeholders = parse_placeholder_defaults(&result.layout_xml, &result.theme);
    result
}

fn parse_theme(xml: &str) -> ThemeContext {
    const COLOR_KEYS: &[&str] = &[
        "dk1", "lt1", "dk2", "lt2", "accent1", "accent2", "accent3", "accent4", "accent5",
        "accent6", "hlink", "folHlink",
    ];
    let mut theme = ThemeContext::default();
    for key in COLOR_KEYS {
        if let Some(color) = parse_theme_color(xml, key) {
            theme.colors.insert((*key).to_string(), color);
        }
    }

    let mut reader = quick_xml::Reader::from_str(xml);
    let mut font_group = None;
    loop {
        match reader.read_event() {
            Ok(quick_xml::events::Event::Start(e)) if e.name().as_ref() == b"a:majorFont" => {
                font_group = Some(true)
            }
            Ok(quick_xml::events::Event::Start(e)) if e.name().as_ref() == b"a:minorFont" => {
                font_group = Some(false)
            }
            Ok(quick_xml::events::Event::Empty(e))
                if font_group.is_some() && e.name().as_ref() == b"a:latin" =>
            {
                let font = x_attr(&e, b"typeface");
                if font_group == Some(true) {
                    theme.major_font = font;
                } else {
                    theme.minor_font = font;
                }
            }
            Ok(quick_xml::events::Event::End(e))
                if e.name().as_ref() == b"a:majorFont" || e.name().as_ref() == b"a:minorFont" =>
            {
                font_group = None
            }
            Ok(quick_xml::events::Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    theme
}

fn apply_color_map(xml: &str, color_map: &mut HashMap<String, String>) {
    let mut reader = quick_xml::Reader::from_str(xml);
    loop {
        match reader.read_event() {
            Ok(quick_xml::events::Event::Start(e)) | Ok(quick_xml::events::Event::Empty(e))
                if e.name().as_ref() == b"p:clrMap"
                    || e.name().as_ref() == b"a:overrideClrMapping" =>
            {
                for attr in e.attributes().flatten() {
                    color_map.insert(
                        String::from_utf8_lossy(attr.key.as_ref()).to_string(),
                        String::from_utf8_lossy(attr.value.as_ref()).to_string(),
                    );
                }
            }
            Ok(quick_xml::events::Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

fn resolve_color_element(
    element: &quick_xml::events::BytesStart<'_>,
    theme: &ThemeContext,
) -> Option<String> {
    match element.name().as_ref() {
        b"a:srgbClr" => x_attr(element, b"val").map(|value| format!("#{value}")),
        b"a:sysClr" => x_attr(element, b"lastClr").map(|value| format!("#{value}")),
        b"a:schemeClr" => x_attr(element, b"val").and_then(|value| theme.resolve_scheme(&value)),
        _ => None,
    }
}

fn placeholder_key(element: &quick_xml::events::BytesStart<'_>) -> PlaceholderKey {
    PlaceholderKey {
        kind: x_attr(element, b"type").unwrap_or_else(|| "obj".to_string()),
        index: x_attr(element, b"idx"),
    }
}

fn find_placeholder(
    placeholders: &HashMap<PlaceholderKey, PlaceholderDefaults>,
    key: &PlaceholderKey,
) -> Option<PlaceholderDefaults> {
    placeholders
        .get(key)
        .or_else(|| {
            key.index.as_ref().and_then(|index| {
                placeholders
                    .iter()
                    .find(|(candidate, _)| candidate.index.as_ref() == Some(index))
                    .map(|(_, value)| value)
            })
        })
        .or_else(|| {
            placeholders
                .iter()
                .find(|(candidate, _)| candidate.kind == key.kind)
                .map(|(_, value)| value)
        })
        .cloned()
}

fn merge_placeholder(target: &mut PlaceholderDefaults, source: &PlaceholderDefaults) {
    if source.has_geometry {
        target.x = source.x;
        target.y = source.y;
        target.w = source.w;
        target.h = source.h;
        target.has_geometry = true;
    }
    for (target, source) in target.text.iter_mut().zip(&source.text) {
        if source.font_size.is_some() {
            target.font_size = source.font_size;
        }
        if source.color.is_some() {
            target.color.clone_from(&source.color);
        }
        if source.font_family.is_some() {
            target.font_family.clone_from(&source.font_family);
        }
        if source.alignment.is_some() {
            target.alignment.clone_from(&source.alignment);
        }
    }
}

fn paragraph_level(element: &quick_xml::events::BytesStart<'_>) -> usize {
    x_attr(element, b"lvl")
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|level| *level < 9)
        .unwrap_or(0)
}

fn level_property_index(name: &[u8]) -> Option<usize> {
    let name = std::str::from_utf8(name).ok()?;
    let level = name.strip_prefix("a:lvl")?.strip_suffix("pPr")?;
    level
        .parse::<usize>()
        .ok()
        .filter(|level| (1..=9).contains(level))
        .map(|level| level - 1)
}

fn parse_placeholder_defaults(
    xml: &str,
    theme: &ThemeContext,
) -> HashMap<PlaceholderKey, PlaceholderDefaults> {
    use quick_xml::events::Event;

    let mut reader = quick_xml::Reader::from_str(xml);
    let mut result = HashMap::new();
    let mut current: Option<(Option<PlaceholderKey>, PlaceholderDefaults)> = None;
    let mut current_level = 0;
    let mut in_level_properties = false;
    let mut in_default_run = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if e.name().as_ref() == b"p:sp" => {
                current = Some((None, PlaceholderDefaults::default()))
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if current.is_some() && e.name().as_ref() == b"p:ph" =>
            {
                current.as_mut().unwrap().0 = Some(placeholder_key(&e));
            }
            Ok(Event::Empty(e)) if current.is_some() && e.name().as_ref() == b"a:off" => {
                let defaults = &mut current.as_mut().unwrap().1;
                defaults.x = x_attr(&e, b"x").and_then(|v| v.parse().ok()).unwrap_or(0);
                defaults.y = x_attr(&e, b"y").and_then(|v| v.parse().ok()).unwrap_or(0);
                defaults.has_geometry = true;
            }
            Ok(Event::Empty(e)) if current.is_some() && e.name().as_ref() == b"a:ext" => {
                let defaults = &mut current.as_mut().unwrap().1;
                defaults.w = x_attr(&e, b"cx").and_then(|v| v.parse().ok()).unwrap_or(0);
                defaults.h = x_attr(&e, b"cy").and_then(|v| v.parse().ok()).unwrap_or(0);
                defaults.has_geometry = true;
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if current.is_some()
                    && (e.name().as_ref() == b"a:defRPr" || e.name().as_ref() == b"a:rPr") =>
            {
                in_default_run = !e.is_empty();
                let text = &mut current.as_mut().unwrap().1.text[current_level];
                text.font_size = x_attr(&e, b"sz")
                    .and_then(|value| value.parse::<f64>().ok())
                    .map(|value| value / 100.0)
                    .or(text.font_size);
            }
            Ok(Event::End(e))
                if e.name().as_ref() == b"a:defRPr" || e.name().as_ref() == b"a:rPr" =>
            {
                in_default_run = false
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if current.is_some() && e.name().as_ref() == b"a:pPr" =>
            {
                current_level = paragraph_level(&e);
                if let Some(value) = x_attr(&e, b"algn") {
                    current.as_mut().unwrap().1.text[current_level].alignment = Some(value);
                }
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if current.is_some() && level_property_index(e.name().as_ref()).is_some() =>
            {
                current_level = level_property_index(e.name().as_ref()).unwrap();
                in_level_properties = !e.is_empty();
                if let Some(value) = x_attr(&e, b"algn") {
                    current.as_mut().unwrap().1.text[current_level].alignment = Some(value);
                }
            }
            Ok(Event::End(e)) if level_property_index(e.name().as_ref()).is_some() => {
                in_level_properties = false;
                current_level = 0;
            }
            Ok(Event::End(e)) if e.name().as_ref() == b"a:pPr" && !in_level_properties => {
                current_level = 0;
            }
            Ok(Event::Empty(e))
                if current.is_some() && in_default_run && e.name().as_ref() == b"a:latin" =>
            {
                if let Some(value) = x_attr(&e, b"typeface").and_then(|v| theme.resolve_font(&v)) {
                    current.as_mut().unwrap().1.text[current_level].font_family = Some(value);
                }
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if current.is_some()
                    && in_default_run
                    && (e.name().as_ref() == b"a:srgbClr"
                        || e.name().as_ref() == b"a:schemeClr"
                        || e.name().as_ref() == b"a:sysClr") =>
            {
                if let Some(color) = resolve_color_element(&e, theme) {
                    current.as_mut().unwrap().1.text[current_level].color = Some(color);
                }
            }
            Ok(Event::End(e)) if e.name().as_ref() == b"p:sp" => {
                if let Some((Some(key), defaults)) = current.take() {
                    result.insert(key, defaults);
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    result
}

fn parse_text_style_defaults(
    xml: &str,
    theme: &ThemeContext,
) -> HashMap<PlaceholderKey, PlaceholderDefaults> {
    use quick_xml::events::Event;

    let mut reader = quick_xml::Reader::from_str(xml);
    let mut result = HashMap::new();
    let mut kind: Option<&str> = None;
    let mut defaults = PlaceholderDefaults::default();
    let mut current_level = None;
    let mut in_default_run = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if e.name().as_ref() == b"p:titleStyle" => kind = Some("title"),
            Ok(Event::Start(e)) if e.name().as_ref() == b"p:bodyStyle" => kind = Some("body"),
            Ok(Event::Start(e)) if e.name().as_ref() == b"p:otherStyle" => kind = Some("other"),
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if kind.is_some() && level_property_index(e.name().as_ref()).is_some() =>
            {
                let level = level_property_index(e.name().as_ref()).unwrap();
                current_level = (!e.is_empty()).then_some(level);
                defaults.text[level].alignment = x_attr(&e, b"algn");
            }
            Ok(Event::End(e)) if level_property_index(e.name().as_ref()).is_some() => {
                current_level = None
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if current_level.is_some() && e.name().as_ref() == b"a:defRPr" =>
            {
                in_default_run = !e.is_empty();
                defaults.text[current_level.unwrap()].font_size = x_attr(&e, b"sz")
                    .and_then(|value| value.parse::<f64>().ok())
                    .map(|value| value / 100.0);
            }
            Ok(Event::End(e)) if e.name().as_ref() == b"a:defRPr" => in_default_run = false,
            Ok(Event::Empty(e)) if in_default_run && e.name().as_ref() == b"a:latin" => {
                defaults.text[current_level.unwrap()].font_family =
                    x_attr(&e, b"typeface").and_then(|value| theme.resolve_font(&value));
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if in_default_run
                    && (e.name().as_ref() == b"a:srgbClr"
                        || e.name().as_ref() == b"a:schemeClr"
                        || e.name().as_ref() == b"a:sysClr") =>
            {
                defaults.text[current_level.unwrap()].color = resolve_color_element(&e, theme);
            }
            Ok(Event::End(e))
                if e.name().as_ref() == b"p:titleStyle"
                    || e.name().as_ref() == b"p:bodyStyle"
                    || e.name().as_ref() == b"p:otherStyle" =>
            {
                if let Some(value) = kind.take() {
                    let keys: &[&str] = match value {
                        "title" => &["title", "ctrTitle"],
                        "body" => &["body", "subTitle"],
                        _ => &["obj"],
                    };
                    for key in keys {
                        result.insert(
                            PlaceholderKey {
                                kind: (*key).to_string(),
                                index: None,
                            },
                            defaults.clone(),
                        );
                    }
                }
                defaults = PlaceholderDefaults::default();
                current_level = None;
                in_default_run = false;
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    result
}

fn format_number(value: f64) -> String {
    if value.fract().abs() < f64::EPSILON {
        format!("{value:.0}")
    } else {
        format!("{value:.2}").trim_end_matches('0').to_string()
    }
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
    placeholder: Option<PlaceholderKey>,
    has_geometry: bool,
    font_size: Option<f64>,
    fill_color: Option<String>,
    border_color: Option<String>,
    border_width: Option<f64>,
    paragraphs: Vec<PptxParagraph>,
    paragraph_levels: Vec<usize>,
    current_paragraph: PptxParagraph,
    current_paragraph_level: usize,
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
            placeholder: None,
            has_geometry: false,
            font_size: None,
            fill_color: None,
            border_color: None,
            border_width: None,
            paragraphs: Vec::new(),
            paragraph_levels: Vec::new(),
            current_paragraph: PptxParagraph::default(),
            current_paragraph_level: 0,
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
    font_family: Option<String>,
}

fn parse_slide_elements(
    bytes: &[u8],
    slide_path: &str,
    xml: &str,
    rels: &std::collections::HashMap<String, String>,
    slide_w: u64,
    slide_h: u64,
    inheritance: &SlideInheritance,
) -> Vec<PptxElement> {
    let mut elements = Vec::new();
    let mut inherited = HashSet::new();
    for element in parse_part_elements(
        bytes,
        &inheritance.master_path,
        &inheritance.master_xml,
        &inheritance.master_rels,
        slide_w,
        slide_h,
        inheritance,
        true,
    )
    .into_iter()
    .chain(parse_part_elements(
        bytes,
        &inheritance.layout_path,
        &inheritance.layout_xml,
        &inheritance.layout_rels,
        slide_w,
        slide_h,
        inheritance,
        true,
    )) {
        if inherited.insert(format!("{element:?}")) {
            elements.push(element);
        }
    }
    elements.extend(parse_part_elements(
        bytes,
        slide_path,
        xml,
        rels,
        slide_w,
        slide_h,
        inheritance,
        false,
    ));
    elements
}

#[allow(clippy::too_many_arguments)]
fn parse_part_elements(
    bytes: &[u8],
    part_path: &str,
    xml: &str,
    rels: &HashMap<String, String>,
    slide_w: u64,
    slide_h: u64,
    inheritance: &SlideInheritance,
    inherited: bool,
) -> Vec<PptxElement> {
    use quick_xml::events::Event;

    let mut reader = quick_xml::Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut elements: Vec<PptxElement> = Vec::new();
    let mut current: Option<ElementBuilder> = None;
    let mut run_accum: Option<RunAccum> = None;
    let mut in_text = false;
    let mut in_rpr = false;
    let mut in_shape_properties = false;
    let mut in_shape_fill = false;
    let mut in_line = false;

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
            Ok(Event::Start(e)) if e.name().as_ref() == b"p:spPr" => in_shape_properties = true,
            Ok(Event::End(e)) if e.name().as_ref() == b"p:spPr" => {
                in_shape_properties = false;
                in_shape_fill = false;
                in_line = false;
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if current.is_some() && e.name().as_ref() == b"p:ph" =>
            {
                current.as_mut().unwrap().placeholder = Some(placeholder_key(&e));
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
                    item.has_geometry = true;
                }
            }
            Ok(Event::Empty(e)) if e.name().as_ref() == b"a:ext" => {
                if let Some(item) = &mut current {
                    item.w = x_attr(&e, b"cx").and_then(|v| v.parse().ok()).unwrap_or(0);
                    item.h = x_attr(&e, b"cy").and_then(|v| v.parse().ok()).unwrap_or(0);
                    item.has_geometry = true;
                }
            }
            Ok(Event::Start(e)) if in_shape_properties && e.name().as_ref() == b"a:solidFill" => {
                in_shape_fill = true
            }
            Ok(Event::End(e)) if e.name().as_ref() == b"a:solidFill" => in_shape_fill = false,
            Ok(Event::Start(e)) if in_shape_properties && e.name().as_ref() == b"a:ln" => {
                in_line = true;
                if let Some(item) = &mut current {
                    item.border_width = x_attr(&e, b"w")
                        .and_then(|value| value.parse::<f64>().ok())
                        .map(|value| value / 12700.0);
                }
            }
            Ok(Event::End(e)) if e.name().as_ref() == b"a:ln" => in_line = false,
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if e.name().as_ref() == b"a:rPr" => {
                in_rpr = !e.is_empty();
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
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if in_rpr
                    && (e.name().as_ref() == b"a:srgbClr"
                        || e.name().as_ref() == b"a:schemeClr"
                        || e.name().as_ref() == b"a:sysClr") =>
            {
                if let Some(color) = resolve_color_element(&e, &inheritance.theme) {
                    if let Some(acc) = &mut run_accum {
                        acc.color = Some(color);
                    }
                }
            }
            Ok(Event::Empty(e)) if in_rpr && e.name().as_ref() == b"a:latin" => {
                if let Some(family) =
                    x_attr(&e, b"typeface").and_then(|value| inheritance.theme.resolve_font(&value))
                {
                    if let Some(acc) = &mut run_accum {
                        acc.font_family = Some(family);
                    }
                }
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if in_shape_properties
                    && (in_shape_fill || in_line)
                    && (e.name().as_ref() == b"a:srgbClr"
                        || e.name().as_ref() == b"a:schemeClr"
                        || e.name().as_ref() == b"a:sysClr") =>
            {
                if let Some(color) = resolve_color_element(&e, &inheritance.theme) {
                    if let Some(item) = &mut current {
                        if in_line {
                            item.border_color = Some(color);
                        } else {
                            item.fill_color = Some(color);
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
                    item.current_paragraph_level = 0;
                }
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if e.name().as_ref() == b"a:pPr" => {
                if let Some(item) = &mut current {
                    item.current_paragraph_level = paragraph_level(&e);
                    if let Some(alignment) = x_attr(&e, b"algn") {
                        item.current_paragraph.alignment = Some(alignment);
                    }
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
                                font_family: acc.font_family,
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
                        item.paragraph_levels.push(item.current_paragraph_level);
                    }
                }
            }
            Ok(Event::End(e)) if e.name().as_ref() == b"p:sp" || e.name().as_ref() == b"p:pic" => {
                if let Some(item) = current.take() {
                    if inherited && item.placeholder.is_some() {
                        continue;
                    }
                    if let Some(element) =
                        finish_element(bytes, part_path, item, rels, slide_w, slide_h, inheritance)
                    {
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
    part_path: &str,
    item: ElementBuilder,
    rels: &std::collections::HashMap<String, String>,
    slide_w: u64,
    slide_h: u64,
    inheritance: &SlideInheritance,
) -> Option<PptxElement> {
    let mut item = item;
    let inherited = item
        .placeholder
        .as_ref()
        .map(|key| inheritance.placeholder(key))
        .unwrap_or_default();
    if !item.has_geometry && inherited.has_geometry {
        item.x = inherited.x;
        item.y = inherited.y;
        item.w = inherited.w;
        item.h = inherited.h;
        item.has_geometry = true;
    }
    for (paragraph, level) in item.paragraphs.iter_mut().zip(&item.paragraph_levels) {
        let inherited_text = &inherited.text[*level];
        if paragraph.alignment.is_none() {
            paragraph.alignment.clone_from(&inherited_text.alignment);
        }
        for run in &mut paragraph.runs {
            if run.font_size.is_none() {
                run.font_size = inherited_text.font_size;
            }
            if run.color.is_none() {
                run.color.clone_from(&inherited_text.color);
            }
            if run.font_family.is_none() {
                run.font_family.clone_from(&inherited_text.font_family);
            }
        }
    }
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
            if item.fill_color.is_none() && item.border_color.is_none() {
                return None;
            }
            let (x, y, w, h) = (item.x, item.y, item.w, item.h);
            if w <= 0 || h <= 0 {
                return None;
            }
            return Some(PptxElement {
                kind: "shape".into(),
                x: x.clamp(0, sw) as u64,
                y: y.clamp(0, sh) as u64,
                w: w.clamp(0, sw.saturating_sub(x)) as u64,
                h: h.clamp(0, sh.saturating_sub(y)) as u64,
                src: None,
                text: None,
                font_size: None,
                paragraphs: None,
                fill_color: item.fill_color,
                border_color: item.border_color,
                border_width: item.border_width,
            });
        }
        let (x, y, w, h) = element_box(&item, &text, sw, sh);
        if w == 0 || h == 0 {
            return None;
        }
        let font_size = item
            .font_size
            .or(inherited.text[0].font_size)
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
            fill_color: item.fill_color,
            border_color: item.border_color,
            border_width: item.border_width,
        });
    }

    if item.kind == "image" && !item.rel_id.is_empty() {
        if let Some(target) = rels.get(&item.rel_id) {
            let path = resolve_part_target(part_path, target);
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
                    fill_color: item.fill_color,
                    border_color: item.border_color,
                    border_width: item.border_width,
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
        let els = parse_slide_elements(
            &[],
            "ppt/slides/slide1.xml",
            xml,
            &rels,
            DEFAULT_SLIDE_W,
            DEFAULT_SLIDE_H,
            &SlideInheritance::default(),
        );
        assert_eq!(els.len(), 1);
        let el = &els[0];
        assert_eq!(el.kind, "text");
        assert_eq!(el.text.as_deref(), Some("Exact deck"));
        assert!(el.x > 0 && el.w > 0);
        let para = el.paragraphs.as_ref().unwrap();
        assert_eq!(para[0].runs[0].text, "Exact deck");
        assert!(para[0].runs[0].bold);
    }

    #[test]
    fn resolves_part_targets_and_theme_colors() {
        assert_eq!(
            resolve_part_target("ppt/slides/slide1.xml", "../slideLayouts/slideLayout7.xml"),
            "ppt/slideLayouts/slideLayout7.xml"
        );
        let master = r#"<p:sldMaster xmlns:p="p" xmlns:a="a"><p:cSld><p:bg><p:bgRef><a:schemeClr val="bg1"/></p:bgRef></p:bg></p:cSld><p:clrMap bg1="lt1"/></p:sldMaster>"#;
        assert_eq!(parse_background_scheme(master).as_deref(), Some("bg1"));
        assert_eq!(parse_color_map(master, "bg1").as_deref(), Some("lt1"));
        let theme = r#"<a:theme xmlns:a="a"><a:themeElements><a:clrScheme><a:lt1><a:sysClr val="window" lastClr="FFFFFF"/></a:lt1></a:clrScheme></a:themeElements></a:theme>"#;
        assert_eq!(parse_theme_color(theme, "lt1").as_deref(), Some("#FFFFFF"));
    }

    #[test]
    fn inherits_placeholder_geometry_and_text_defaults() {
        let theme_xml = r#"<a:theme xmlns:a="a"><a:themeElements><a:clrScheme><a:accent1><a:srgbClr val="336699"/></a:accent1></a:clrScheme><a:fontScheme><a:majorFont><a:latin typeface="Aptos Display"/></a:majorFont></a:fontScheme></a:themeElements></a:theme>"#;
        let theme = parse_theme(theme_xml);
        let layout = r#"<p:sldLayout xmlns:p="p" xmlns:a="a"><p:cSld><p:spTree><p:sp><p:nvSpPr><p:ph type="title" idx="1"/></p:nvSpPr><p:spPr><a:xfrm><a:off x="100" y="200"/><a:ext cx="300" cy="400"/></a:xfrm></p:spPr><p:txBody><a:p><a:pPr algn="ctr"><a:defRPr sz="3200"><a:solidFill><a:schemeClr val="accent1"/></a:solidFill><a:latin typeface="+mj-lt"/></a:defRPr></a:pPr></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:sldLayout>"#;
        let inheritance = SlideInheritance {
            layout_placeholders: parse_placeholder_defaults(layout, &theme),
            theme,
            ..Default::default()
        };
        let slide = r#"<p:sld xmlns:p="p" xmlns:a="a"><p:cSld><p:spTree><p:sp><p:nvSpPr><p:cNvPr name="Heading"/><p:ph type="title" idx="1"/></p:nvSpPr><p:spPr/><p:txBody><a:p><a:r><a:t>Inherited</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:sld>"#;
        let elements = parse_slide_elements(
            &[],
            "ppt/slides/slide1.xml",
            slide,
            &HashMap::new(),
            1000,
            1000,
            &inheritance,
        );
        let element = &elements[0];
        assert_eq!(
            (element.x, element.y, element.w, element.h),
            (100, 200, 300, 400)
        );
        let paragraph = &element.paragraphs.as_ref().unwrap()[0];
        assert_eq!(paragraph.alignment.as_deref(), Some("ctr"));
        assert_eq!(paragraph.runs[0].font_size, Some(32.0));
        assert_eq!(paragraph.runs[0].color.as_deref(), Some("#336699"));
        assert_eq!(
            paragraph.runs[0].font_family.as_deref(),
            Some("Aptos Display")
        );
    }

    #[test]
    fn inherits_text_defaults_for_matching_paragraph_level() {
        let master = r#"<p:sldMaster xmlns:p="p" xmlns:a="a"><p:txStyles><p:bodyStyle><a:lvl1pPr algn="l"><a:defRPr sz="4000"/></a:lvl1pPr><a:lvl2pPr algn="r"><a:defRPr sz="2000"/></a:lvl2pPr></p:bodyStyle></p:txStyles></p:sldMaster>"#;
        let layout = r#"<p:sldLayout xmlns:p="p" xmlns:a="a"><p:cSld><p:spTree><p:sp><p:nvSpPr><p:ph type="body" idx="1"/></p:nvSpPr><p:txBody><a:lstStyle><a:lvl2pPr><a:defRPr><a:solidFill><a:srgbClr val="123456"/></a:solidFill></a:defRPr></a:lvl2pPr></a:lstStyle></p:txBody></p:sp></p:spTree></p:cSld></p:sldLayout>"#;
        let theme = ThemeContext::default();
        let inheritance = SlideInheritance {
            master_text_defaults: parse_text_style_defaults(master, &theme),
            layout_placeholders: parse_placeholder_defaults(layout, &theme),
            ..Default::default()
        };
        let slide = r#"<p:sld xmlns:p="p" xmlns:a="a"><p:cSld><p:spTree><p:sp><p:nvSpPr><p:cNvPr name="Body"/><p:ph type="body" idx="1"/></p:nvSpPr><p:txBody><a:p><a:pPr lvl="1"/><a:r><a:t>Nested</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:sld>"#;

        let elements = parse_slide_elements(
            &[],
            "ppt/slides/slide1.xml",
            slide,
            &HashMap::new(),
            1000,
            1000,
            &inheritance,
        );
        let paragraph = &elements[0].paragraphs.as_ref().unwrap()[0];
        assert_eq!(paragraph.alignment.as_deref(), Some("r"));
        assert_eq!(paragraph.runs[0].font_size, Some(20.0));
        assert_eq!(paragraph.runs[0].color.as_deref(), Some("#123456"));
    }

    #[test]
    fn placeholder_without_type_defaults_to_obj() {
        let mut reader = quick_xml::Reader::from_str(r#"<p:ph xmlns:p="p" idx="7"/>"#);
        let key = loop {
            match reader.read_event().unwrap() {
                quick_xml::events::Event::Empty(element) => break placeholder_key(&element),
                _ => continue,
            }
        };

        assert_eq!(key.kind, "obj");
        assert_eq!(key.index.as_deref(), Some("7"));
    }

    #[test]
    fn parses_shape_fill_border_and_scheme_run_color() {
        let mut inheritance = SlideInheritance::default();
        inheritance
            .theme
            .colors
            .insert("accent2".into(), "#ABCDEF".into());
        let xml = r#"<p:sld xmlns:p="p" xmlns:a="a"><p:cSld><p:spTree><p:sp><p:nvSpPr><p:cNvPr name="Box"/></p:nvSpPr><p:spPr><a:xfrm><a:off x="10" y="20"/><a:ext cx="300" cy="200"/></a:xfrm><a:solidFill><a:schemeClr val="accent2"/></a:solidFill><a:ln w="25400"><a:solidFill><a:srgbClr val="112233"/></a:solidFill></a:ln></p:spPr><p:txBody><a:p><a:r><a:rPr><a:solidFill><a:schemeClr val="accent2"/></a:solidFill></a:rPr><a:t>Styled</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:sld>"#;
        let elements = parse_slide_elements(
            &[],
            "ppt/slides/slide1.xml",
            xml,
            &HashMap::new(),
            1000,
            1000,
            &inheritance,
        );
        assert_eq!(elements[0].fill_color.as_deref(), Some("#ABCDEF"));
        assert_eq!(elements[0].border_color.as_deref(), Some("#112233"));
        assert_eq!(elements[0].border_width, Some(2.0));
        assert_eq!(
            elements[0].paragraphs.as_ref().unwrap()[0].runs[0]
                .color
                .as_deref(),
            Some("#ABCDEF")
        );
    }

    #[test]
    fn parses_gradient_background_as_css() {
        let xml = r#"<p:sld xmlns:p="p" xmlns:a="a"><p:cSld><p:bg><p:bgPr><a:gradFill><a:gsLst><a:gs pos="0"><a:srgbClr val="000000"/></a:gs><a:gs pos="100000"><a:srgbClr val="FFFFFF"/></a:gs></a:gsLst><a:lin ang="5400000"/></a:gradFill></p:bgPr></p:bg></p:cSld></p:sld>"#;
        let background = parse_slide_background(xml).unwrap();
        assert_eq!(background.kind, "gradient");
        assert_eq!(
            background.gradient.as_deref(),
            Some("linear-gradient(180deg, #000000 0%, #FFFFFF 100%)")
        );
    }

    #[test]
    fn merges_non_placeholder_master_and_layout_artwork_in_order() {
        let master = r#"<p:sldMaster xmlns:p="p" xmlns:a="a"><p:cSld><p:spTree>
            <p:sp><p:nvSpPr><p:cNvPr name="Master art"/></p:nvSpPr><p:spPr><a:xfrm><a:off x="10" y="10"/><a:ext cx="100" cy="100"/></a:xfrm><a:solidFill><a:srgbClr val="112233"/></a:solidFill></p:spPr></p:sp>
            <p:sp><p:nvSpPr><p:cNvPr name="Master prompt"/><p:ph type="body"/></p:nvSpPr><p:spPr><a:xfrm><a:off x="1" y="1"/><a:ext cx="100" cy="100"/></a:xfrm></p:spPr><p:txBody><a:p><a:r><a:t>Click to edit Master text styles</a:t></a:r></a:p></p:txBody></p:sp>
        </p:spTree></p:cSld></p:sldMaster>"#;
        let layout = r#"<p:sldLayout xmlns:p="p" xmlns:a="a"><p:cSld><p:spTree>
            <p:sp><p:nvSpPr><p:cNvPr name="Duplicate art"/></p:nvSpPr><p:spPr><a:xfrm><a:off x="10" y="10"/><a:ext cx="100" cy="100"/></a:xfrm><a:solidFill><a:srgbClr val="112233"/></a:solidFill></p:spPr></p:sp>
            <p:sp><p:nvSpPr><p:cNvPr name="Layout art"/></p:nvSpPr><p:spPr><a:xfrm><a:off x="20" y="20"/><a:ext cx="100" cy="100"/></a:xfrm><a:solidFill><a:srgbClr val="445566"/></a:solidFill></p:spPr></p:sp>
        </p:spTree></p:cSld></p:sldLayout>"#;
        let slide = r#"<p:sld xmlns:p="p" xmlns:a="a"><p:cSld><p:spTree>
            <p:sp><p:nvSpPr><p:cNvPr name="Slide art"/></p:nvSpPr><p:spPr><a:xfrm><a:off x="30" y="30"/><a:ext cx="100" cy="100"/></a:xfrm><a:solidFill><a:srgbClr val="778899"/></a:solidFill></p:spPr></p:sp>
        </p:spTree></p:cSld></p:sld>"#;
        let inheritance = SlideInheritance {
            master_path: "ppt/slideMasters/slideMaster1.xml".into(),
            master_xml: master.into(),
            layout_path: "ppt/slideLayouts/slideLayout1.xml".into(),
            layout_xml: layout.into(),
            ..Default::default()
        };

        let elements = parse_slide_elements(
            &[],
            "ppt/slides/slide1.xml",
            slide,
            &HashMap::new(),
            1000,
            1000,
            &inheritance,
        );

        assert_eq!(elements.len(), 3);
        assert_eq!(elements[0].fill_color.as_deref(), Some("#112233"));
        assert_eq!(elements[1].fill_color.as_deref(), Some("#445566"));
        assert_eq!(elements[2].fill_color.as_deref(), Some("#778899"));
        assert!(elements.iter().all(|element| element.text.is_none()));
    }

    #[test]
    fn inherited_images_use_their_part_relationships() {
        use std::io::Write;

        let mut bytes = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut bytes));
            zip.start_file(
                "ppt/media/master.png",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(b"master image").unwrap();
            zip.start_file(
                "ppt/media/layout.png",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(b"layout image").unwrap();
            zip.finish().unwrap();
        }
        let picture = |rel: &str, x: u64| {
            format!(
                r#"<p:sld xmlns:p="p" xmlns:a="a" xmlns:r="r"><p:cSld><p:spTree><p:pic><p:nvPicPr><p:cNvPr name="Artwork"/></p:nvPicPr><p:blipFill><a:blip r:embed="{rel}"/></p:blipFill><p:spPr><a:xfrm><a:off x="{x}" y="10"/><a:ext cx="100" cy="100"/></a:xfrm></p:spPr></p:pic></p:spTree></p:cSld></p:sld>"#
            )
        };
        let inheritance = SlideInheritance {
            master_path: "ppt/slideMasters/slideMaster1.xml".into(),
            master_xml: picture("rIdMaster", 10),
            master_rels: HashMap::from([("rIdMaster".into(), "../media/master.png".into())]),
            layout_path: "ppt/slideLayouts/slideLayout1.xml".into(),
            layout_xml: picture("rIdLayout", 20),
            layout_rels: HashMap::from([("rIdLayout".into(), "../media/layout.png".into())]),
            ..Default::default()
        };

        let elements = parse_slide_elements(
            &bytes,
            "ppt/slides/slide1.xml",
            "<p:sld xmlns:p=\"p\"/>",
            &HashMap::new(),
            1000,
            1000,
            &inheritance,
        );

        assert_eq!(elements.len(), 2);
        assert_eq!(elements[0].kind, "image");
        assert_eq!(elements[1].kind, "image");
        assert_ne!(elements[0].src, elements[1].src);
    }
}
