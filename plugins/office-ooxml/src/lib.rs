//! Optional enhanced Office OOXML plugin payload.
//!
//! This crate deliberately lives under `plugins/` so `ooxmlsdk` does not become
//! part of ViewIt's base APK. Android loads this as an optional JNI sidecar from
//! the installed plugin ZIP.

use std::io::{Cursor, Read};

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
    Ok(json!({
        "kind": "docx",
        "blocks": blocks,
        "byte_len": bytes.len()
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

    Ok(json!({
        "kind": "xlsx",
        "sheets": sheets,
        "byte_len": bytes.len()
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

    let mut slides = Vec::new();
    for name in slide_names {
        let xml = zip_entry_string(bytes, &name)?;
        let texts = parse_drawing_texts(&xml)?;
        let title = texts.first().cloned().unwrap_or_default();
        let body = texts.iter().skip(1).cloned().collect::<Vec<_>>().join("\n");
        slides.push(json!({ "title": title, "body": body }));
    }

    Ok(json!({
        "kind": "pptx",
        "slide_count": slides.len(),
        "slides": slides,
        "byte_len": bytes.len(),
        "asset_path": ""
    })
    .to_string())
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
    #[test]
    fn rejects_non_ooxml_extension() {
        assert!(super::inspect_ooxml(&[], "txt").is_err());
    }
}
