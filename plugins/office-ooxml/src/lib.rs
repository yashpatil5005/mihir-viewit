//! Optional enhanced Office OOXML plugin payload.
//!
//! This crate deliberately lives under `plugins/` so `ooxmlsdk` does not become
//! part of ViewIt's base APK. Android loads this as an optional JNI sidecar from
//! the installed plugin ZIP.

use std::io::{Cursor, Read};

use base64::Engine;
use jni::objects::{JByteArray, JClass, JString};
use jni::sys::jstring;
use jni::JNIEnv;
use ooxmlsdk::parts::presentation_document::PresentationDocument;
use ooxmlsdk::parts::spreadsheet_document::SpreadsheetDocument;
use ooxmlsdk::parts::wordprocessing_document::WordprocessingDocument;
use quick_xml::events::Event;
use quick_xml::name::QName;
use quick_xml::Reader;
use serde::Serialize;
use serde_json::json;
use zip::ZipArchive;

const DEFAULT_SLIDE_W: i64 = 9_144_000;
const DEFAULT_SLIDE_H: i64 = 5_143_500;

#[derive(Debug, Serialize)]
pub struct OoxmlInspection {
    pub format: String,
    pub readable: bool,
    pub main_part: bool,
}

pub fn inspect_ooxml(bytes: &[u8], ext: &str) -> Result<OoxmlInspection, String> {
    match ext.to_ascii_lowercase().as_str() {
        "docx" | "docm" => {
            let document = WordprocessingDocument::new(Cursor::new(bytes.to_vec()))
                .map_err(|e| format!("wordprocessing open: {e}"))?;
            Ok(OoxmlInspection {
                format: "wordprocessing".into(),
                readable: true,
                main_part: document.main_document_part().is_ok(),
            })
        }
        "xlsx" | "xlsm" => {
            let document = SpreadsheetDocument::new(Cursor::new(bytes.to_vec()))
                .map_err(|e| format!("spreadsheet open: {e}"))?;
            Ok(OoxmlInspection {
                format: "spreadsheet".into(),
                readable: true,
                main_part: document.workbook_part().is_ok(),
            })
        }
        "pptx" | "pptm" => {
            let document = PresentationDocument::new(Cursor::new(bytes.to_vec()))
                .map_err(|e| format!("presentation open: {e}"))?;
            Ok(OoxmlInspection {
                format: "presentation".into(),
                readable: true,
                main_part: document.presentation_part().is_ok(),
            })
        }
        _ => Err(format!("unsupported OOXML extension: {ext}")),
    }
}

pub fn render_ooxml_document(bytes: &[u8], ext: &str) -> Result<String, String> {
    inspect_ooxml(bytes, ext)?;
    match ext.to_ascii_lowercase().as_str() {
        "docx" | "docm" => render_docx(bytes),
        "xlsx" | "xlsm" => render_xlsx(bytes),
        "pptx" | "pptm" => render_pptx(bytes),
        _ => Err(format!("unsupported OOXML extension: {ext}")),
    }
}

fn render_docx(bytes: &[u8]) -> Result<String, String> {
    let xml = zip_entry_string(bytes, "word/document.xml")?;

    let blocks = parse_docx_blocks(&xml)?;
    let mut warnings = Vec::new();
    if !zip_has_entry(bytes, "word/styles.xml") {
        warnings.push("styles.xml missing — style-based heading inference may be incomplete".to_string());
    }
    if zip_has_entry(bytes, "word/header1.xml") || zip_has_entry(bytes, "word/footer1.xml") {
        warnings.push("headers/footers present but not yet rendered".to_string());
    }
    let searchable = collect_docx_search_text(&blocks);
    Ok(json!({
        "kind": "docx",
        "blocks": blocks,
        "byte_len": bytes.len(),
        "warnings": warnings,
        "fidelity": {
            "level": "text+structure",
            "supports": ["text", "headings", "lists", "tables"],
            "missing": ["images", "footnotes", "headers", "footers", "tracked-changes", "styles"],
        },
        "search_text": searchable,
    })
    .to_string())
}

fn render_xlsx(bytes: &[u8]) -> Result<String, String> {
    let workbook = zip_entry_string(bytes, "xl/workbook.xml")?;
    let workbook_rels = zip_entry_string(bytes, "xl/_rels/workbook.xml.rels")?;
    let shared_strings = match zip_entry_string(bytes, "xl/sharedStrings.xml") {
        Ok(xml) => parse_shared_strings(&xml)?,
        Err(_) => Vec::new(),
    };
    let rels = parse_relationships(&workbook_rels)?;
    let mut sheets = Vec::new();
    let mut warnings = Vec::new();

    for (name, rel_id) in parse_workbook_sheets(&workbook)? {
        let target = rels
            .get(&rel_id)
            .ok_or_else(|| format!("workbook relationship missing: {rel_id}"))?;
        let path = if target.starts_with("xl/") {
            target.clone()
        } else {
            format!("xl/{}", target.trim_start_matches('/'))
        };
        let sheet_xml = zip_entry_string(bytes, &path)?;
        let rows = parse_sheet_rows(&sheet_xml, &shared_strings)?;
        let total_cols = rows.iter().map(Vec::len).max().unwrap_or(0);
        let header = rows.first().cloned().unwrap_or_default();
        let preview_rows = rows.into_iter().skip(1).take(200).collect::<Vec<_>>();
        let total_rows = preview_rows.len() + usize::from(!header.is_empty());
        sheets.push(json!({
            "name": name,
            "header": header,
            "preview_rows": preview_rows,
            "total_rows_hint": total_rows,
            "total_cols_hint": total_cols
        }));
    }

    if zip_has_entry(bytes, "xl/drawings/") {
        warnings.push("sheet drawings/charts present but not yet rendered".to_string());
    }

    Ok(json!({
        "kind": "xlsx",
        "sheets": sheets,
        "byte_len": bytes.len(),
        "warnings": warnings,
        "fidelity": {
            "level": "text+structure",
            "supports": ["text", "sheets", "shared-strings"],
            "missing": ["formatting", "formulas", "merged-cells", "charts", "filters", "frozen-panes", "cell-styles"],
        },
    })
    .to_string())
}

fn render_pptx(bytes: &[u8]) -> Result<String, String> {
    let archive = ZipArchive::new(Cursor::new(bytes)).map_err(|e| format!("zip open: {e}"))?;
    let mut slide_names = archive
        .file_names()
        .filter(|name| name.starts_with("ppt/slides/slide") && name.ends_with(".xml"))
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    slide_names.sort_by_key(|name| {
        name.trim_start_matches("ppt/slides/slide")
            .trim_end_matches(".xml")
            .parse::<u32>()
            .unwrap_or(u32::MAX)
    });
    drop(archive);

    let (slide_w, slide_h) = parse_presentation_size(bytes).unwrap_or((DEFAULT_SLIDE_W, DEFAULT_SLIDE_H));

    let mut slides = Vec::new();
    let warnings: Vec<String> = Vec::new();
    for name in &slide_names {
        let xml = zip_entry_string(bytes, name)?;
        let rels_path = slide_relationships_path(name);
        let rels = zip_entry_string(bytes, &rels_path)
            .ok()
            .map(|xml| parse_relationships(&xml))
            .transpose()?
            .unwrap_or_default();
        let parsed = parse_pptx_slide_layout(bytes, &xml, &rels, slide_w, slide_h)?;
        let bg = parse_slide_background(&xml);
        let texts = if parsed.texts.is_empty() {
            parse_drawing_texts(&xml)?
        } else {
            parsed.texts
        };
        let title = texts.first().cloned().unwrap_or_default();
        let body = texts.iter().skip(1).cloned().collect::<Vec<_>>().join("\n");
        let mut slide = serde_json::json!({
            "title": title,
            "body": body,
            "width": slide_w,
            "height": slide_h,
            "elements": parsed.elements,
        });
        if let Some(background) = bg {
            slide.as_object_mut().unwrap().insert("background".to_string(), background);
        }
        slides.push(slide);
    }

    Ok(serde_json::json!({
        "kind": "pptx",
        "slide_count": slides.len(),
        "slides": slides,
        "byte_len": bytes.len(),
        "asset_path": "",
        "warnings": warnings,
        "fidelity": {
            "level": "text+layout+bullets+rich-runs",
            "supports": ["text", "slide-elements", "images", "positions", "font-sizes", "bold", "italic", "underline", "bullets", "backgrounds"],
            "missing": ["master-layout-inheritance", "theme-fonts", "theme-colors", "rich-text-colors", "shapes", "charts", "tables", "smartart", "animations", "transitions"],
        },
    })
    .to_string())
}

struct ParsedSlideLayout {
    texts: Vec<String>,
    elements: Vec<serde_json::Value>,
}

#[derive(Default)]
struct PptxElementBuilder {
    kind: &'static str,
    name: String,
    x: i64,
    y: i64,
    w: i64,
    h: i64,
    text: String,
    rel_id: String,
    font_size: Option<f64>,
    paragraphs: Vec<PptxParagraphBuilder>,
    current_paragraph: PptxParagraphBuilder,
}

#[derive(Default, Clone)]
struct PptxParagraphBuilder {
    runs: Vec<PptxRunBuilder>,
    bullet: bool,
}

#[derive(Default, Clone, Serialize)]
struct PptxRunBuilder {
    text: String,
    bold: bool,
    italic: bool,
    underline: bool,
    font_size: Option<f64>,
    color: Option<String>,
}

struct PptxRunAccum {
    text: String,
    bold: bool,
    italic: bool,
    underline: bool,
    font_size: Option<f64>,
    color: Option<String>,
}

fn parse_presentation_size(bytes: &[u8]) -> Result<(i64, i64), String> {
    let xml = zip_entry_string(bytes, "ppt/presentation.xml")?;
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if e.name() == QName(b"p:sldSz") => {
                let w = attr_value(&e, b"cx")
                    .and_then(|v| v.parse::<i64>().ok())
                    .unwrap_or(DEFAULT_SLIDE_W);
                let h = attr_value(&e, b"cy")
                    .and_then(|v| v.parse::<i64>().ok())
                    .unwrap_or(DEFAULT_SLIDE_H);
                return Ok((w, h));
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("presentation size parse: {e}")),
            _ => {}
        }
    }
    Ok((DEFAULT_SLIDE_W, DEFAULT_SLIDE_H))
}

fn parse_slide_background(xml: &str) -> Option<serde_json::Value> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut bg = None;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if e.name() == QName(b"p:bg") => {
                // Found background element
            }
            Ok(Event::Start(e)) if e.name() == QName(b"a:solidFill") => {
                // Start of solid fill
            }
            Ok(Event::Empty(e)) if e.name() == QName(b"a:srgbClr") => {
                if let Some(color) = attr_value(&e, b"val") {
                    bg = Some(serde_json::json!({
                        "type": "solid",
                        "color": format!("#{}", color),
                    }));
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    bg
}

fn slide_relationships_path(slide_path: &str) -> String {
    let file = slide_path.rsplit('/').next().unwrap_or(slide_path);
    format!("ppt/slides/_rels/{file}.rels")
}

fn parse_pptx_slide_layout(
    bytes: &[u8],
    xml: &str,
    rels: &std::collections::HashMap<String, String>,
    slide_w: i64,
    slide_h: i64,
) -> Result<ParsedSlideLayout, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut elements = Vec::new();
    let mut texts = Vec::new();
    let mut current: Option<PptxElementBuilder> = None;
    let mut in_text = false;
    let mut run_accum: Option<PptxRunAccum> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if e.name() == QName(b"p:sp") => {
                current = Some(PptxElementBuilder { kind: "text", ..Default::default() });
            }
            Ok(Event::Start(e)) if e.name() == QName(b"p:pic") => {
                current = Some(PptxElementBuilder { kind: "image", ..Default::default() });
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if e.name() == QName(b"p:cNvPr") => {
                if let Some(item) = &mut current {
                    item.name = attr_value(&e, b"name").unwrap_or_default();
                }
            }
            Ok(Event::Empty(e)) if e.name() == QName(b"a:off") => {
                if let Some(item) = &mut current {
                    item.x = attr_value(&e, b"x").and_then(|v| v.parse().ok()).unwrap_or(0);
                    item.y = attr_value(&e, b"y").and_then(|v| v.parse().ok()).unwrap_or(0);
                }
            }
            Ok(Event::Empty(e)) if e.name() == QName(b"a:ext") => {
                if let Some(item) = &mut current {
                    item.w = attr_value(&e, b"cx").and_then(|v| v.parse().ok()).unwrap_or(0);
                    item.h = attr_value(&e, b"cy").and_then(|v| v.parse().ok()).unwrap_or(0);
                }
            }
            Ok(Event::Empty(e)) if e.name() == QName(b"a:rPr") => {
                if let Some(item) = &mut current {
                    if item.font_size.is_none() {
                        item.font_size = attr_value(&e, b"sz")
                            .and_then(|v| v.parse::<f64>().ok())
                            .map(|v| v / 100.0);
                    }
                }
            }
            Ok(Event::Start(e)) if e.name() == QName(b"a:rPr") => {
                if let Some(item) = &mut current {
                    if item.font_size.is_none() {
                        item.font_size = attr_value(&e, b"sz")
                            .and_then(|v| v.parse::<f64>().ok())
                            .map(|v| v / 100.0);
                    }
                }
                if let Some(acc) = &mut run_accum {
                    if acc.font_size.is_none() {
                        acc.font_size = attr_value(&e, b"sz")
                            .and_then(|v| v.parse::<f64>().ok())
                            .map(|v| v / 100.0);
                    }
                    if e.attributes().flatten().any(|a| a.key.as_ref() == b"b" && a.value.as_ref() == b"1") {
                        acc.bold = true;
                    }
                    if e.attributes().flatten().any(|a| a.key.as_ref() == b"i" && a.value.as_ref() == b"1") {
                        acc.italic = true;
                    }
                    if e.attributes().flatten().any(|a| a.key.as_ref() == b"u" && a.value.as_ref() == b"sng") {
                        acc.underline = true;
                    }
                }
            }
            Ok(Event::Empty(e)) if e.name() == QName(b"a:blip") => {
                if let Some(item) = &mut current {
                    item.rel_id = attr_value(&e, b"r:embed").unwrap_or_default();
                }
            }
            Ok(Event::Start(e)) if e.name() == QName(b"a:buChar") || e.name() == QName(b"a:buNone") => {
                if let Some(item) = &mut current {
                    item.current_paragraph.bullet = true;
                }
            }
            Ok(Event::Start(e)) if e.name() == QName(b"a:p") => {
                if let Some(item) = &mut current {
                    item.current_paragraph = PptxParagraphBuilder::default();
                }
            }
            Ok(Event::Start(e)) if e.name() == QName(b"a:r") => {
                run_accum = Some(PptxRunAccum {
                    text: String::new(),
                    bold: false,
                    italic: false,
                    underline: false,
                    font_size: None,
                    color: None,
                });
            }
            Ok(Event::Start(e)) if e.name() == QName(b"a:t") => in_text = true,
            Ok(Event::Text(e)) if in_text => {
                let decoded = e.decode().map_err(|e| format!("slide text decode: {e}"))?;
                if let Some(acc) = &mut run_accum {
                    acc.text.push_str(&decoded);
                } else if let Some(item) = &mut current {
                    item.text.push_str(&decoded);
                }
            }
            Ok(Event::End(e)) if e.name() == QName(b"a:t") => in_text = false,
            Ok(Event::End(e)) if e.name() == QName(b"a:r") => {
                if let Some(acc) = run_accum.take() {
                    if let Some(item) = &mut current {
                        item.current_paragraph.runs.push(PptxRunBuilder {
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
            Ok(Event::End(e)) if e.name() == QName(b"a:p") => {
                if let Some(item) = &mut current {
                    let para = std::mem::take(&mut item.current_paragraph);
                    if !para.runs.is_empty() || para.bullet {
                        item.paragraphs.push(para);
                    }
                }
            }
            Ok(Event::End(e)) if e.name() == QName(b"p:sp") || e.name() == QName(b"p:pic") => {
                if let Some(item) = current.take() {
                    if let Some(element) = finish_pptx_element(bytes, item, rels, slide_w, slide_h)? {
                        if element.get("kind").and_then(|v| v.as_str()) == Some("text") {
                            if let Some(text) = element.get("text").and_then(|v| v.as_str()) {
                                texts.push(text.to_string());
                            }
                        }
                        elements.push(element);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("slide layout parse: {e}")),
            _ => {}
        }
    }

    Ok(ParsedSlideLayout { texts, elements })
}

fn finish_pptx_element(
    bytes: &[u8],
    item: PptxElementBuilder,
    rels: &std::collections::HashMap<String, String>,
    slide_w: i64,
    slide_h: i64,
) -> Result<Option<serde_json::Value>, String> {
    if item.kind == "text" {
        let mut flat_text = String::new();
        let mut paragraphs = Vec::new();
        for para in &item.paragraphs {
            let mut para_text = String::new();
            let mut runs = Vec::new();
            if para.runs.is_empty() {
                if para.bullet {
                    para_text.push_str("• ");
                }
            }
            for run in &para.runs {
                if !run.text.is_empty() {
                    para_text.push_str(&run.text);
                    let mut run_obj = serde_json::Map::new();
                    run_obj.insert("text".to_string(), serde_json::Value::String(run.text.clone()));
                    if run.bold { run_obj.insert("bold".to_string(), serde_json::Value::Bool(true)); }
                    if run.italic { run_obj.insert("italic".to_string(), serde_json::Value::Bool(true)); }
                    if run.underline { run_obj.insert("underline".to_string(), serde_json::Value::Bool(true)); }
                    if let Some(fs) = run.font_size { run_obj.insert("font_size".to_string(), serde_json::json!(fs)); }
                    if let Some(c) = &run.color { run_obj.insert("color".to_string(), serde_json::Value::String(c.clone())); }
                    runs.push(serde_json::Value::Object(run_obj));
                }
            }
            if !para_text.is_empty() {
                flat_text.push_str(&para_text);
                flat_text.push('\n');
            }
            let mut para_obj = serde_json::Map::new();
            para_obj.insert("runs".to_string(), serde_json::Value::Array(runs));
            if para.bullet { para_obj.insert("bullet".to_string(), serde_json::Value::Bool(true)); }
            paragraphs.push(serde_json::Value::Object(para_obj));
        }
        let text = if flat_text.is_empty() {
            item.text.trim().to_string()
        } else {
            flat_text.trim_end().to_string()
        };
        if text.is_empty() {
            return Ok(None);
        }
        let (x, y, w, h) = element_box(&item, &text, slide_w, slide_h);
        if w == 0 || h == 0 {
            return Ok(None);
        }
        let mut obj = serde_json::Map::new();
        obj.insert("kind".to_string(), serde_json::Value::String("text".to_string()));
        obj.insert("text".to_string(), serde_json::Value::String(text));
        obj.insert("x".to_string(), serde_json::json!(x));
        obj.insert("y".to_string(), serde_json::json!(y));
        obj.insert("w".to_string(), serde_json::json!(w));
        obj.insert("h".to_string(), serde_json::json!(h));
        obj.insert("font_size".to_string(), serde_json::json!(item.font_size.unwrap_or_else(|| fallback_font_size(&item.name, &item.text))));
        if !paragraphs.is_empty() {
            obj.insert("paragraphs".to_string(), serde_json::Value::Array(paragraphs));
        }
        return Ok(Some(serde_json::Value::Object(obj)));
    }

    if item.kind == "image" && !item.rel_id.is_empty() {
        let x = item.x.max(0).min(slide_w);
        let y = item.y.max(0).min(slide_h);
        let w = item.w.max(0).min(slide_w.saturating_sub(x));
        let h = item.h.max(0).min(slide_h.saturating_sub(y));
        if w == 0 || h == 0 {
            return Ok(None);
        }
        if let Some(target) = rels.get(&item.rel_id) {
            let path = normalize_pptx_target(target);
            if let Ok(data) = zip_entry_bytes(bytes, &path) {
                let mime = image_mime(&path);
                let encoded = base64::engine::general_purpose::STANDARD.encode(data);
                return Ok(Some(json!({
                    "kind": "image",
                    "src": format!("data:{mime};base64,{encoded}"),
                    "x": x,
                    "y": y,
                    "w": w,
                    "h": h,
                })));
            }
        }
    }

    Ok(None)
}

fn element_box(item: &PptxElementBuilder, text: &str, slide_w: i64, slide_h: i64) -> (i64, i64, i64, i64) {
    let x = item.x.max(0).min(slide_w);
    let y = item.y.max(0).min(slide_h);
    let w = item.w.max(0).min(slide_w.saturating_sub(x));
    let h = item.h.max(0).min(slide_h.saturating_sub(y));
    if w > 0 && h > 0 {
        return (x, y, w, h);
    }
    fallback_text_box(&item.name, text, slide_w, slide_h)
}

fn fallback_text_box(name: &str, text: &str, slide_w: i64, slide_h: i64) -> (i64, i64, i64, i64) {
    let lower = name.to_ascii_lowercase();
    if lower.contains("title") || text.len() <= 80 {
        return (
            slide_w * 7 / 100,
            slide_h * 8 / 100,
            slide_w * 86 / 100,
            slide_h * 18 / 100,
        );
    }
    (
        slide_w * 9 / 100,
        slide_h * 30 / 100,
        slide_w * 82 / 100,
        slide_h * 58 / 100,
    )
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
    match path.rsplit('.').next().unwrap_or_default().to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        _ => "image/png",
    }
}

fn zip_entry_bytes(bytes: &[u8], name: &str) -> Result<Vec<u8>, String> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(|e| format!("zip open: {e}"))?;
    let mut data = Vec::new();
    archive
        .by_name(name)
        .map_err(|e| format!("{name}: {e}"))?
        .read_to_end(&mut data)
        .map_err(|e| format!("{name} read: {e}"))?;
    Ok(data)
}

fn zip_entry_string(bytes: &[u8], name: &str) -> Result<String, String> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(|e| format!("zip open: {e}"))?;
    let mut xml = String::new();
    archive
        .by_name(name)
        .map_err(|e| format!("{name}: {e}"))?
        .read_to_string(&mut xml)
        .map_err(|e| format!("{name} read: {e}"))?;
    Ok(xml)
}

fn zip_has_entry(bytes: &[u8], name: &str) -> bool {
    ZipArchive::new(Cursor::new(bytes))
        .map(|mut a| a.by_name(name).is_ok())
        .unwrap_or(false)
}

fn collect_docx_search_text(blocks: &[serde_json::Value]) -> Vec<String> {
    let mut out = Vec::new();
    for block in blocks {
        let kind = block.get("kind").and_then(|v| v.as_str()).unwrap_or("");
        if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
            if !text.is_empty() {
                out.push(text.to_string());
            }
        }
        if kind == "table" {
            if let Some(rows) = block.get("rows").and_then(|v| v.as_array()) {
                for row in rows {
                    if let Some(cells) = row.as_array() {
                        for cell in cells {
                            if let Some(s) = cell.as_str() {
                                if !s.is_empty() {
                                    out.push(s.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    out
}

fn attr_value(e: &quick_xml::events::BytesStart<'_>, key: &[u8]) -> Option<String> {
    e.attributes().flatten().find_map(|attr| {
        if attr.key.as_ref() == key {
            Some(String::from_utf8_lossy(attr.value.as_ref()).to_string())
        } else {
            None
        }
    })
}

fn parse_relationships(xml: &str) -> Result<std::collections::HashMap<String, String>, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut rels = std::collections::HashMap::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if e.name().as_ref() == b"Relationship" => {
                if let (Some(id), Some(target)) = (attr_value(&e, b"Id"), attr_value(&e, b"Target")) {
                    rels.insert(id, target);
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("relationships parse: {e}")),
            _ => {}
        }
    }
    Ok(rels)
}

fn parse_workbook_sheets(xml: &str) -> Result<Vec<(String, String)>, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut sheets = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if e.name() == QName(b"sheet") => {
                let name = attr_value(&e, b"name").unwrap_or_else(|| "Sheet".to_string());
                let rel_id = attr_value(&e, b"r:id").ok_or("sheet missing r:id")?;
                sheets.push((name, rel_id));
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("workbook parse: {e}")),
            _ => {}
        }
    }
    Ok(sheets)
}

fn parse_shared_strings(xml: &str) -> Result<Vec<String>, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut strings = Vec::new();
    let mut current = String::new();
    let mut in_si = false;
    let mut in_text = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if e.name() == QName(b"si") => {
                in_si = true;
                current.clear();
            }
            Ok(Event::Start(e)) if e.name() == QName(b"t") => in_text = true,
            Ok(Event::Text(e)) if in_text => current.push_str(&e.decode().map_err(|e| format!("shared string decode: {e}"))?),
            Ok(Event::End(e)) if e.name() == QName(b"t") => in_text = false,
            Ok(Event::End(e)) if e.name() == QName(b"si") => {
                in_si = false;
                strings.push(current.clone());
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("sharedStrings parse: {e}")),
            _ => {}
        }
    }
    if in_si && !current.is_empty() {
        strings.push(current);
    }
    Ok(strings)
}

fn parse_sheet_rows(xml: &str, shared_strings: &[String]) -> Result<Vec<Vec<String>>, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut cell_type = String::new();
    let mut cell_ref = String::new();
    let mut cell_value = String::new();
    let mut in_value = false;
    let mut in_inline_text = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if e.name() == QName(b"row") => row.clear(),
            Ok(Event::Start(e)) if e.name() == QName(b"c") => {
                cell_type = attr_value(&e, b"t").unwrap_or_default();
                cell_ref = attr_value(&e, b"r").unwrap_or_default();
                cell_value.clear();
            }
            Ok(Event::Start(e)) if e.name() == QName(b"v") => in_value = true,
            Ok(Event::Start(e)) if e.name() == QName(b"t") => in_inline_text = true,
            Ok(Event::Text(e)) if in_value || in_inline_text => {
                cell_value.push_str(&e.decode().map_err(|e| format!("sheet text decode: {e}"))?);
            }
            Ok(Event::End(e)) if e.name() == QName(b"v") => in_value = false,
            Ok(Event::End(e)) if e.name() == QName(b"t") => in_inline_text = false,
            Ok(Event::End(e)) if e.name() == QName(b"c") => {
                let col = column_index(&cell_ref).unwrap_or(row.len());
                if row.len() <= col {
                    row.resize(col + 1, String::new());
                }
                row[col] = if cell_type == "s" {
                    cell_value
                        .parse::<usize>()
                        .ok()
                        .and_then(|idx| shared_strings.get(idx).cloned())
                        .unwrap_or_else(|| cell_value.clone())
                } else {
                    cell_value.clone()
                };
            }
            Ok(Event::End(e)) if e.name() == QName(b"row") => {
                if row.iter().any(|cell| !cell.is_empty()) {
                    rows.push(std::mem::take(&mut row));
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("sheet parse: {e}")),
            _ => {}
        }
    }
    Ok(rows)
}

fn column_index(cell_ref: &str) -> Option<usize> {
    let mut value = 0usize;
    let mut saw_letter = false;
    for b in cell_ref.bytes().take_while(|b| b.is_ascii_alphabetic()) {
        saw_letter = true;
        value = value * 26 + usize::from(b.to_ascii_uppercase() - b'A' + 1);
    }
    saw_letter.then_some(value.saturating_sub(1))
}

fn parse_drawing_texts(xml: &str) -> Result<Vec<String>, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut texts = Vec::new();
    let mut current = String::new();
    let mut in_paragraph = false;
    let mut in_text = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if e.name() == QName(b"a:p") => {
                in_paragraph = true;
                current.clear();
            }
            Ok(Event::Start(e)) if e.name() == QName(b"a:t") => in_text = true,
            Ok(Event::Text(e)) if in_text => current.push_str(&e.decode().map_err(|e| format!("slide text decode: {e}"))?),
            Ok(Event::End(e)) if e.name() == QName(b"a:t") => in_text = false,
            Ok(Event::End(e)) if e.name() == QName(b"a:p") => {
                let text = current.trim();
                if !text.is_empty() {
                    texts.push(text.to_string());
                }
                in_paragraph = false;
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("slide parse: {e}")),
            _ => {}
        }
    }
    if in_paragraph && !current.trim().is_empty() {
        texts.push(current.trim().to_string());
    }
    Ok(texts)
}

fn parse_docx_blocks(xml: &str) -> Result<Vec<serde_json::Value>, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut blocks = Vec::new();
    let mut paragraph_text = String::new();
    let mut table_rows: Vec<Vec<String>> = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut cell_text = String::new();
    let mut heading: Option<u8> = None;
    let mut list_item = false;
    let mut in_paragraph = false;
    let mut in_table = false;
    let mut in_cell = false;
    let mut in_text = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => match e.name() {
                QName(b"w:p") => {
                    in_paragraph = true;
                    paragraph_text.clear();
                    heading = None;
                    list_item = false;
                }
                QName(b"w:tbl") => {
                    in_table = true;
                    table_rows.clear();
                }
                QName(b"w:tr") if in_table => row.clear(),
                QName(b"w:tc") if in_table => {
                    in_cell = true;
                    cell_text.clear();
                }
                QName(b"w:t") => in_text = true,
                QName(b"w:numPr") if in_paragraph => list_item = true,
                QName(b"w:pStyle") if in_paragraph => {
                    for attr in e.attributes().flatten() {
                        if attr.key == QName(b"w:val") {
                            let value = String::from_utf8_lossy(attr.value.as_ref()).to_string();
                            heading = heading_from_style(&value);
                        }
                    }
                }
                _ => {}
            },
            Ok(Event::Text(e)) if in_text => {
                let text = e.decode().map_err(|e| format!("text decode: {e}"))?;
                if in_cell {
                    cell_text.push_str(&text);
                } else if in_paragraph {
                    paragraph_text.push_str(&text);
                }
            }
            Ok(Event::End(e)) => match e.name() {
                QName(b"w:t") => in_text = false,
                QName(b"w:p") => {
                    if !in_table {
                        let text = paragraph_text.trim();
                        if !text.is_empty() {
                            blocks.push(if list_item {
                                json!({ "kind": "list-item", "text": text, "level": 0 })
                            } else {
                                json!({ "kind": "paragraph", "text": text, "heading": heading })
                            });
                        }
                    }
                    in_paragraph = false;
                }
                QName(b"w:tc") if in_table => {
                    row.push(cell_text.trim().to_string());
                    in_cell = false;
                }
                QName(b"w:tr") if in_table => {
                    if !row.is_empty() {
                        table_rows.push(std::mem::take(&mut row));
                    }
                }
                QName(b"w:tbl") => {
                    if !table_rows.is_empty() {
                        blocks.push(json!({ "kind": "table", "rows": table_rows }));
                    }
                    table_rows = Vec::new();
                    in_table = false;
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("word/document.xml parse: {e}")),
            _ => {}
        }
    }

    Ok(blocks)
}

fn heading_from_style(style: &str) -> Option<u8> {
    if style.eq_ignore_ascii_case("title") {
        return Some(1);
    }
    let lower = style.to_ascii_lowercase();
    lower
        .strip_prefix("heading")
        .and_then(|n| n.parse::<u8>().ok())
        .filter(|n| (1..=6).contains(n))
}

#[no_mangle]
pub extern "system" fn Java_ai_viewit_plugins_office_1ooxml_Plugin_renderNative(
    mut env: JNIEnv,
    _class: JClass,
    bytes: JByteArray,
    ext: JString,
) -> jstring {
    let result = (|| -> Result<String, String> {
        let bytes = env
            .convert_byte_array(bytes)
            .map_err(|e| format!("byte array: {e}"))?;
        let ext: String = env
            .get_string(&ext)
            .map_err(|e| format!("extension: {e}"))?
            .into();
        render_ooxml_document(&bytes, &ext)
    })();

    let output = match result {
        Ok(json) => json,
        Err(error) => json!({ "error": error }).to_string(),
    };

    env.new_string(output)
        .expect("failed to allocate JNI string")
        .into_raw()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_ooxml_extension() {
        assert!(super::inspect_ooxml(&[], "txt").is_err());
    }

    #[test]
    fn docx_output_has_warnings_and_fidelity() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:r><w:t>Hello world</w:t></w:r></w:p>
  </w:body>
</w:document>"#;
        let mut buf = Vec::new();
        {
            use std::io::Write;
            let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            zip.start_file("word/document.xml", zip::write::SimpleFileOptions::default()).unwrap();
            zip.write_all(xml.as_bytes()).unwrap();
            zip.finish().unwrap();
        }
        let out = super::render_docx(&buf).unwrap();
        let val: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(val["kind"], "docx");
        assert!(val.get("warnings").is_some(), "docx output must include warnings array");
        assert!(val.get("fidelity").is_some(), "docx output must include fidelity summary");
        assert!(val["blocks"].is_array());
    }

    #[test]
    fn xlsx_output_has_warnings_and_fidelity() {
        let workbook = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets><sheet name="Sheet1" sheetId="1" r:id="rId1"/></sheets>
</workbook>"#;
        let rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
</Relationships>"#;
        let sheet = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData>
    <row r="1"><c r="A1" t="inlineStr"><is><t>Header</t></is></c></row>
    <row r="2"><c r="A2"><v>42</v></c></row>
  </sheetData>
</worksheet>"#;
        let mut buf = Vec::new();
        {
            use std::io::Write;
            let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            zip.start_file("xl/workbook.xml", zip::write::SimpleFileOptions::default()).unwrap();
            zip.write_all(workbook.as_bytes()).unwrap();
            zip.start_file("xl/_rels/workbook.xml.rels", zip::write::SimpleFileOptions::default()).unwrap();
            zip.write_all(rels.as_bytes()).unwrap();
            zip.start_file("xl/worksheets/sheet1.xml", zip::write::SimpleFileOptions::default()).unwrap();
            zip.write_all(sheet.as_bytes()).unwrap();
            zip.finish().unwrap();
        }
        let out = super::render_xlsx(&buf).unwrap();
        let val: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(val["kind"], "xlsx");
        assert!(val.get("warnings").is_some(), "xlsx output must include warnings array");
        assert!(val.get("fidelity").is_some(), "xlsx output must include fidelity summary");
    }
}
