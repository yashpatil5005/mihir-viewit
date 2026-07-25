//! Optional enhanced Office OOXML plugin payload.
//!
//! This crate deliberately lives under `plugins/` so `ooxmlsdk` does not become
//! part of ViewIt's base APK. Android loads this as an optional JNI sidecar from
//! the installed plugin ZIP.

use std::io::{Cursor, Read};

use base64::Engine;
use calamine::{open_workbook_auto_from_rs, Data, Reader as SpreadsheetReader};
use jni::objects::{JByteArray, JClass, JString};
use jni::sys::jstring;
use jni::JNIEnv;
use quick_xml::events::Event;
use quick_xml::name::QName;
use quick_xml::Reader;
use serde::Serialize;
use serde_json::json;
use zip::ZipArchive;

const DEFAULT_SLIDE_W: i64 = 9_144_000;
const DEFAULT_SLIDE_H: i64 = 5_143_500;

fn plugin_renderer_marker() -> serde_json::Value {
    json!({
        "id": "office-ooxml",
        "label": "Office OOXML Plugin Renderer",
        "theme": "plugin-pink",
    })
}

fn escape_html(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

fn office_html_document(title: &str, body: &str) -> String {
    format!(
        r#"<section class="office-plugin-html"><style>
.office-plugin-html {{ color:#161616; background:#f4f1ea; padding:16px; overflow:auto; }}
.office-plugin-page {{ max-width:860px; margin:0 auto 18px; background:white; box-shadow:0 10px 30px rgba(0,0,0,.14); border:1px solid #ddd4c4; padding:48px; min-height:720px; }}
.office-plugin-page h1,.office-plugin-page h2,.office-plugin-page h3 {{ color:#111; margin:1.1em 0 .45em; line-height:1.18; }}
.office-plugin-page p {{ margin:.55em 0; line-height:1.58; }}
.office-plugin-page ul {{ margin:.55em 0; padding-left:1.6em; }}
.office-plugin-page table {{ border-collapse:collapse; width:100%; margin:1em 0; }}
.office-plugin-page td,.office-plugin-page th {{ border:1px solid #bbb; padding:6px 8px; vertical-align:top; }}
.office-plugin-workbook {{ background:#0f172a; color:#e5e7eb; min-height:70vh; padding:12px; overflow:auto; }}
.office-plugin-sheet {{ margin:0 0 18px; background:#fff; color:#111; border-radius:8px; overflow:hidden; box-shadow:0 8px 24px rgba(0,0,0,.22); }}
.office-plugin-sheet h2 {{ margin:0; padding:10px 12px; background:#1d6f42; color:white; font-size:15px; }}
.office-plugin-grid {{ border-collapse:separate; border-spacing:0; width:max-content; min-width:100%; font:13px ui-monospace,SFMono-Regular,Menlo,Consolas,monospace; }}
.office-plugin-grid th {{ position:sticky; top:0; background:#e7f1e9; z-index:2; font-weight:700; }}
.office-plugin-grid td,.office-plugin-grid th {{ border-right:1px solid #cbd5cf; border-bottom:1px solid #cbd5cf; padding:5px 8px; white-space:pre; max-width:280px; overflow:hidden; text-overflow:ellipsis; }}
.office-plugin-slides {{ background:#111827; color:white; padding:14px; }}
.office-plugin-slide {{ position:relative; margin:0 auto 18px; max-width:960px; aspect-ratio:16/9; background:white; color:#111; overflow:hidden; box-shadow:0 12px 34px rgba(0,0,0,.35); }}
.office-plugin-slide-title {{ position:absolute; left:6%; top:6%; right:6%; font-size:clamp(18px,3vw,34px); font-weight:700; }}
.office-plugin-slide-body {{ position:absolute; left:7%; top:23%; right:7%; bottom:8%; white-space:pre-wrap; font-size:clamp(13px,1.7vw,21px); line-height:1.35; }}
@media (max-width:700px) {{ .office-plugin-page {{ padding:22px; min-height:0; }} }}
</style><div data-office-plugin-title="{}">{}</div></section>"#,
        escape_html(title),
        body
    )
}

#[derive(Debug, Serialize)]
pub struct OoxmlInspection {
    pub format: String,
    pub readable: bool,
    pub main_part: bool,
}

pub fn inspect_ooxml(bytes: &[u8], ext: &str) -> Result<OoxmlInspection, String> {
    match ext.to_ascii_lowercase().as_str() {
        "docx" | "docm" => {
            let main_part = zip_has_entry(bytes, "word/document.xml");
            Ok(OoxmlInspection {
                format: "wordprocessing".into(),
                readable: main_part,
                main_part,
            })
        }
        "xlsx" | "xlsm" | "xls" => {
            let workbook = open_workbook_auto_from_rs(Cursor::new(bytes.to_vec()))
                .map_err(|e| format!("spreadsheet open: {e}"))?;
            let main_part = !workbook.sheet_names().is_empty();
            Ok(OoxmlInspection {
                format: "spreadsheet".into(),
                readable: true,
                main_part,
            })
        }
        "pptx" | "pptm" => {
            let main_part = zip_has_entry(bytes, "ppt/presentation.xml");
            Ok(OoxmlInspection {
                format: "presentation".into(),
                readable: main_part,
                main_part,
            })
        }
        _ => Err(format!("unsupported OOXML extension: {ext}")),
    }
}

pub fn render_ooxml_document(bytes: &[u8], ext: &str) -> Result<String, String> {
    inspect_ooxml(bytes, ext)?;
    match ext.to_ascii_lowercase().as_str() {
        "docx" | "docm" => render_docx(bytes),
        "xlsx" | "xlsm" | "xls" => render_xlsx(bytes),
        "pptx" | "pptm" => render_pptx(bytes),
        _ => Err(format!("unsupported OOXML extension: {ext}")),
    }
}

fn render_docx(bytes: &[u8]) -> Result<String, String> {
    let xml = zip_entry_string(bytes, "word/document.xml")?;
    let rels_xml = zip_entry_string(bytes, "word/_rels/document.xml.rels").unwrap_or_default();
    let rels = if rels_xml.is_empty() {
        std::collections::HashMap::new()
    } else {
        parse_relationships(&rels_xml)?
    };

    let blocks = parse_docx_blocks(bytes, &xml, &rels)?;
    let mut warnings = Vec::new();
    if !zip_has_entry(bytes, "word/styles.xml") {
        warnings.push("styles.xml missing — style-based heading inference may be incomplete".to_string());
    }
    if zip_has_entry(bytes, "word/header1.xml") || zip_has_entry(bytes, "word/footer1.xml") {
        warnings.push("headers/footers present but not yet rendered".to_string());
    }
    if zip_has_entry(bytes, "word/footnotes.xml") {
        warnings.push("footnotes present but not yet rendered".to_string());
    }
    if zip_has_entry(bytes, "word/comments.xml") {
        warnings.push("comments present but not yet rendered".to_string());
    }
    let searchable = collect_docx_search_text(&blocks);
    let html = office_html_document("DOCX", &docx_blocks_to_html(&blocks));
    let mut supports = vec!["text", "headings", "lists", "tables"];
    let mut missing = vec!["footnotes", "headers", "footers", "tracked-changes", "styles"];
    let has_images = blocks.iter().any(|b| b.get("kind").and_then(|v| v.as_str()) == Some("image"));
    let has_links = blocks.iter().any(|b| b.get("kind").and_then(|v| v.as_str()) == Some("hyperlink"));
    if has_images { supports.push("images"); } else { missing.push("images"); }
    if has_links { supports.push("hyperlinks"); } else { missing.push("hyperlinks"); }
    Ok(json!({
        "kind": "docx",
        "renderer": plugin_renderer_marker(),
        "html": html,
        "blocks": blocks,
        "byte_len": bytes.len(),
        "warnings": warnings,
        "fidelity": {
            "level": "text+structure",
            "supports": supports,
            "missing": missing,
        },
        "search_text": searchable,
    })
    .to_string())
}

fn docx_blocks_to_html(blocks: &[serde_json::Value]) -> String {
    let mut html = String::from("<article class=\"office-plugin-page\">");
    for block in blocks {
        match block.get("kind").and_then(|v| v.as_str()).unwrap_or("") {
            "paragraph" => {
                let text = escape_html(block.get("text").and_then(|v| v.as_str()).unwrap_or(""));
                if let Some(level) = block.get("heading").and_then(|v| v.as_u64()) {
                    let level = level.clamp(1, 6);
                    html.push_str(&format!("<h{level}>{text}</h{level}>"));
                } else {
                    html.push_str(&format!("<p>{text}</p>"));
                }
            }
            "list-item" => {
                let text = escape_html(block.get("text").and_then(|v| v.as_str()).unwrap_or(""));
                html.push_str(&format!("<ul><li>{text}</li></ul>"));
            }
            "hyperlink" => {
                let text = escape_html(block.get("text").and_then(|v| v.as_str()).unwrap_or(""));
                let href = escape_html(block.get("href").and_then(|v| v.as_str()).unwrap_or("#"));
                html.push_str(&format!("<p><a href=\"{href}\">{text}</a></p>"));
            }
            "table" => {
                html.push_str("<table><tbody>");
                if let Some(rows) = block.get("rows").and_then(|v| v.as_array()) {
                    for row in rows {
                        html.push_str("<tr>");
                        if let Some(cells) = row.as_array() {
                            for cell in cells {
                                html.push_str("<td>");
                                html.push_str(&escape_html(cell.as_str().unwrap_or("")));
                                html.push_str("</td>");
                            }
                        }
                        html.push_str("</tr>");
                    }
                }
                html.push_str("</tbody></table>");
            }
            "image" => html.push_str("<p><em>[Embedded image]</em></p>"),
            _ => {}
        }
    }
    html.push_str("</article>");
    html
}

fn render_xlsx(bytes: &[u8]) -> Result<String, String> {
    let mut sheets = Vec::new();
    let mut warnings = Vec::new();
    let mut html_body = String::from("<div class=\"office-plugin-workbook\">");
    let mut workbook = open_workbook_auto_from_rs(Cursor::new(bytes.to_vec()))
        .map_err(|e| format!("spreadsheet open: {e}"))?;
    let sheet_names = workbook.sheet_names().to_vec();
    let sheet_paths = xlsx_sheet_paths(bytes).unwrap_or_default();

    for name in sheet_names {
        let range = workbook
            .worksheet_range(&name)
            .map_err(|e| format!("sheet read {name}: {e}"))?;
        let rows = range
            .rows()
            .map(|row| row.iter().map(data_to_string).collect::<Vec<_>>())
            .collect::<Vec<_>>();
        let total_rows = rows.len();
        let total_cols = rows.iter().map(Vec::len).max().unwrap_or(0);
        let header = rows.first().cloned().unwrap_or_else(|| {
            (0..total_cols).map(|i| spreadsheet_column_label(i)).collect()
        });
        let preview_rows = rows.iter().skip(1).take(500).cloned().collect::<Vec<_>>();

        let merged_cells;
        let frozen_panes;
        if let Some(path) = sheet_paths.get(&name) {
            let sheet_xml = zip_entry_string(bytes, path).unwrap_or_default();
            merged_cells = parse_sheet_merged_cells(&sheet_xml);
            frozen_panes = parse_sheet_frozen_panes(&sheet_xml);
        } else {
            merged_cells = Vec::new();
            frozen_panes = None;
        }

        let mut sheet_obj = json!({
            "name": name,
            "header": header,
            "preview_rows": preview_rows,
            "total_rows_hint": total_rows,
            "total_cols_hint": total_cols,
        });
        if !merged_cells.is_empty() {
            sheet_obj.as_object_mut().unwrap().insert("merged_cells".to_string(), json!(merged_cells));
        }
        if let Some(fp) = frozen_panes {
            sheet_obj.as_object_mut().unwrap().insert("frozen_panes".to_string(), json!(fp));
        }

        html_body.push_str(&format!("<section class=\"office-plugin-sheet\"><h2>{}</h2><table class=\"office-plugin-grid\"><thead><tr><th></th>", escape_html(&name)));
        for col in 0..total_cols {
            html_body.push_str(&format!("<th>{}</th>", spreadsheet_column_label(col)));
        }
        html_body.push_str("</tr></thead><tbody>");
        for (row_index, row) in rows.iter().take(500).enumerate() {
            html_body.push_str(&format!("<tr><th>{}</th>", row_index + 1));
            for col in 0..total_cols {
                let value = row.get(col).map_or("", String::as_str);
                html_body.push_str(&format!("<td>{}</td>", escape_html(value)));
            }
            html_body.push_str("</tr>");
        }
        html_body.push_str("</tbody></table></section>");
        sheets.push(sheet_obj);
    }
    html_body.push_str("</div>");

    warnings.push("spreadsheet rendered with calamine; charts, pivot tables, macros, and exact Excel layout are not rendered".to_string());

    let supports = vec!["xls", "xlsx", "xlsm", "multi-sheet", "typed-cells", "dates", "formulas-as-values"];
    let missing = vec!["charts", "pivot-tables", "macros", "exact-cell-styles", "page-layout"];

    Ok(json!({
        "kind": "xlsx",
        "renderer": plugin_renderer_marker(),
        "html": office_html_document("Workbook", &html_body),
        "sheets": sheets,
        "byte_len": bytes.len(),
        "warnings": warnings,
        "fidelity": {
            "level": "text+structure",
            "supports": supports,
            "missing": missing,
        },
    })
    .to_string())
}

fn xlsx_sheet_paths(bytes: &[u8]) -> Result<std::collections::HashMap<String, String>, String> {
    let workbook = zip_entry_string(bytes, "xl/workbook.xml")?;
    let workbook_rels = zip_entry_string(bytes, "xl/_rels/workbook.xml.rels")?;
    let rels = parse_relationships(&workbook_rels)?;
    let mut paths = std::collections::HashMap::new();
    for (name, rel_id) in parse_workbook_sheets(&workbook)? {
        if let Some(target) = rels.get(&rel_id) {
            let path = if target.starts_with("xl/") {
                target.clone()
            } else {
                format!("xl/{}", target.trim_start_matches('/'))
            };
            paths.insert(name, path);
        }
    }
    Ok(paths)
}

fn data_to_string(data: &Data) -> String {
    match data {
        Data::Empty => String::new(),
        Data::String(s) => s.clone(),
        Data::Float(v) => {
            if v.fract() == 0.0 { format!("{v:.0}") } else { v.to_string() }
        }
        Data::Int(v) => v.to_string(),
        Data::Bool(v) => v.to_string(),
        Data::DateTime(v) => v.to_string(),
        Data::DateTimeIso(v) => v.clone(),
        Data::DurationIso(v) => v.clone(),
        Data::Error(v) => format!("{v:?}"),
    }
}

fn spreadsheet_column_label(index: usize) -> String {
    let mut n = index + 1;
    let mut label = String::new();
    while n > 0 {
        let rem = (n - 1) % 26;
        label.insert(0, (b'A' + rem as u8) as char);
        n = (n - 1) / 26;
    }
    label
}

fn parse_sheet_merged_cells(xml: &str) -> Vec<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut merged = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Empty(e)) if e.name() == QName(b"mergeCell") => {
                if let Some(ref_range) = attr_value(&e, b"ref") {
                    merged.push(ref_range);
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    merged
}

fn parse_sheet_frozen_panes(xml: &str) -> Option<serde_json::Value> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut found_pane = false;
    let mut x_split = 0u32;
    let mut y_split = 0u32;
    let mut active_pane = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Empty(e)) if e.name() == QName(b"pane") => {
                found_pane = true;
                x_split = attr_value(&e, b"xSplit").and_then(|v| v.parse().ok()).unwrap_or(0);
                y_split = attr_value(&e, b"ySplit").and_then(|v| v.parse().ok()).unwrap_or(0);
                active_pane = attr_value(&e, b"activePane").unwrap_or_default();
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    if found_pane && (x_split > 0 || y_split > 0) {
        Some(json!({
            "x_split": x_split,
            "y_split": y_split,
            "active_pane": active_pane,
        }))
    } else {
        None
    }
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
    let mut html_body = String::from("<div class=\"office-plugin-slides\">");
    for (idx, slide) in slides.iter().enumerate() {
        let title = escape_html(slide.get("title").and_then(|v| v.as_str()).unwrap_or(""));
        let body = escape_html(slide.get("body").and_then(|v| v.as_str()).unwrap_or(""));
        html_body.push_str(&format!(
            "<section class=\"office-plugin-slide\" aria-label=\"Slide {}\"><div class=\"office-plugin-slide-title\">{}</div><div class=\"office-plugin-slide-body\">{}</div></section>",
            idx + 1,
            title,
            body
        ));
    }
    html_body.push_str("</div>");

    Ok(serde_json::json!({
        "kind": "pptx",
        "renderer": plugin_renderer_marker(),
        "html": office_html_document("Presentation", &html_body),
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

fn parse_docx_blocks(
    bytes: &[u8],
    xml: &str,
    rels: &std::collections::HashMap<String, String>,
) -> Result<Vec<serde_json::Value>, String> {
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
    let mut hyperlink_rel_id = String::new();
    let mut in_hyperlink = false;
    let mut in_drawing = false;
    let mut drawing_blip_embed = String::new();

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
                QName(b"w:hyperlink") => {
                    in_hyperlink = true;
                    hyperlink_rel_id = attr_value(&e, b"r:id").unwrap_or_default();
                }
                QName(b"w:drawing") => {
                    in_drawing = true;
                    drawing_blip_embed = String::new();
                }
                QName(b"a:blip") if in_drawing => {
                    drawing_blip_embed = attr_value(&e, b"r:embed").unwrap_or_default();
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
                QName(b"w:hyperlink") => {
                    if !hyperlink_rel_id.is_empty() && !paragraph_text.trim().is_empty() {
                        if let Some(target) = rels.get(&hyperlink_rel_id) {
                            blocks.push(json!({
                                "kind": "hyperlink",
                                "text": paragraph_text.trim(),
                                "href": target,
                            }));
                        }
                    }
                    in_hyperlink = false;
                    hyperlink_rel_id = String::new();
                    paragraph_text.clear();
                }
                QName(b"w:drawing") => {
                    if !drawing_blip_embed.is_empty() {
                        if let Some(target) = rels.get(&drawing_blip_embed) {
                            let path = normalize_docx_target(target);
                            if let Ok(data) = zip_entry_bytes(bytes, &path) {
                                let mime = image_mime(&path);
                                let encoded = base64::engine::general_purpose::STANDARD.encode(&data);
                                blocks.push(json!({
                                    "kind": "image",
                                    "src": format!("data:{};base64,{}", mime, encoded),
                                    "byte_len": data.len(),
                                }));
                            }
                        }
                    }
                    in_drawing = false;
                    drawing_blip_embed = String::new();
                }
                QName(b"w:p") => {
                    if !in_table && !in_hyperlink {
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

fn normalize_docx_target(target: &str) -> String {
    let clean = target.trim_start_matches('/');
    if clean.starts_with("word/") {
        clean.to_string()
    } else if clean.starts_with("../") {
        format!("word/{}", clean.trim_start_matches("../"))
    } else {
        format!("word/{}", clean)
    }
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

    #[test]
    fn docx_parses_hyperlinks() {
        let document = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body>
    <w:p><w:hyperlink r:id="rId1"><w:r><w:t>Click here</w:t></w:r></w:hyperlink></w:p>
  </w:body>
</w:document>"#;
        let rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com"/>
</Relationships>"#;
        let mut buf = Vec::new();
        {
            use std::io::Write;
            let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            zip.start_file("word/document.xml", zip::write::SimpleFileOptions::default()).unwrap();
            zip.write_all(document.as_bytes()).unwrap();
            zip.start_file("word/_rels/document.xml.rels", zip::write::SimpleFileOptions::default()).unwrap();
            zip.write_all(rels.as_bytes()).unwrap();
            zip.finish().unwrap();
        }
        let out = super::render_docx(&buf).unwrap();
        let val: serde_json::Value = serde_json::from_str(&out).unwrap();
        let blocks = val["blocks"].as_array().unwrap();
        let link = blocks.iter().find(|b| b["kind"] == "hyperlink").expect("hyperlink block");
        assert_eq!(link["text"], "Click here");
        assert_eq!(link["href"], "https://example.com");
    }

    #[test]
    fn xlsx_parses_merged_cells() {
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
  <mergeCells count="1"><mergeCell ref="A1:B2"/></mergeCells>
  <sheetData>
    <row r="1"><c r="A1" t="inlineStr"><is><t>Merged</t></is></c></row>
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
        let sheet = &val["sheets"][0];
        assert!(sheet.get("merged_cells").is_some(), "xlsx output must include merged_cells");
        assert_eq!(sheet["merged_cells"][0], "A1:B2");
    }

    #[test]
    fn xlsx_parses_frozen_panes() {
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
  <sheetViews><sheetView tabSelected="1" workbookViewId="0">
    <pane xSplit="2" ySplit="1" topLeftCell="C2" activePane="bottomRight" state="frozen"/>
    <selection pane="bottomRight" activeCell="C2" sqref="C2"/>
  </sheetView></sheetViews>
  <sheetData>
    <row r="1"><c r="A1" t="inlineStr"><is><t>Header</t></is></c></row>
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
        let sheet = &val["sheets"][0];
        assert!(sheet.get("frozen_panes").is_some(), "xlsx output must include frozen_panes");
        assert_eq!(sheet["frozen_panes"]["x_split"], 2);
        assert_eq!(sheet["frozen_panes"]["y_split"], 1);
    }
}
