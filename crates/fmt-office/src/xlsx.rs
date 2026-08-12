//! XLSX / XLSB / XLS parser — unified for office-universal.
//!
//! Uses calamine for cell data (well-known spreadsheet engine) and enriches the
//! sheet preview with real worksheet fidelity read from the OOXML parts:
//! merged cells, frozen panes, column widths, and cell formulas. This is the
//! "office plugin" path — via `viewit_fmt_office::render` — so it ships in the
//! optional plugin, not the base APK.

use calamine::{Data, Reader, Xls, Xlsb, Xlsx};
use std::collections::HashMap;
use std::io::Cursor;
use viewit_core_types::{Document, Error, XlsxFrozenPanes, XlsxSheet};

// Include city: not used as a global here; keep module small.

pub fn parse_xlsx(bytes: &[u8]) -> Result<Document, Error> {
    use viewit_core_types::Document;

    let mut workbook = Xlsx::<Cursor<Vec<u8>>>::new(Cursor::new(bytes.to_vec()))
        .map_err(|e| Error::Parse(format!("calamine xlsx: {e}")))?;
    let sheets_meta = workbook.worksheets();
    let paths = xlsx_worksheet_paths(bytes).unwrap_or_default();

    let mut sheets: Vec<XlsxSheet> = Vec::with_capacity(sheets_meta.len().min(8));
    for (name, range) in sheets_meta.into_iter().take(8) {
        let mut rows_iter = range.rows();
        let header: Vec<String> = rows_iter
            .next()
            .map(|r| r.iter().map(cell_to_string).collect())
            .unwrap_or_default();
        let preview_rows: Vec<Vec<String>> = rows_iter
            .take(200)
            .map(|r| r.iter().map(cell_to_string).collect())
            .collect();

        // Enrich from the worksheet part (merges / frozen panes / widths / f).
        let mut merged_cells: Option<Vec<String>> = None;
        let mut frozen_panes: Option<XlsxFrozenPanes> = None;
        let mut column_widths: Option<Vec<Option<f64>>> = None;
        let mut preview_formulas: Option<Vec<Vec<String>>> = None;

        if let Some(path) = paths.get(&name) {
            if let Ok(xml) = zip_entry_string(bytes, path) {
                let merges = parse_sheet_merged_cells(&xml);
                if !merges.is_empty() {
                    merged_cells = Some(merges);
                }
                frozen_panes = parse_sheet_frozen_pane(&xml);
                let widths = parse_sheet_col_widths(&xml);
                if !widths.is_empty() {
                    column_widths = Some(widths);
                }
                let formulas = parse_sheet_formulas(&xml);
                if !formulas.is_empty() {
                    preview_formulas = Some(align_formulas(header.len(), &preview_rows, &formulas));
                }
            }
        }

        sheets.push(XlsxSheet {
            name,
            header,
            preview_rows,
            total_rows_hint: Some(range.height()),
            total_cols_hint: Some(range.width()),
            preview_formulas,
            merged_cells,
            frozen_panes,
            column_widths,
        });
    }

    Ok(Document::Xlsx {
        sheets,
        byte_len: bytes.len(),
    })
}

pub fn parse_xlsm(bytes: &[u8]) -> Result<Document, Error> {
    // XLSM shares the same OOXML worksheet structure as XLSX.
    parse_xlsx(bytes)
}

pub fn parse_xlsb(bytes: &[u8]) -> Result<Document, Error> {
    use viewit_core_types::Document;

    let mut workbook = Xlsb::<Cursor<Vec<u8>>>::new(Cursor::new(bytes.to_vec()))
        .map_err(|e| Error::Parse(format!("calamine xlsb: {e}")))?;
    let sheets_meta = workbook.worksheets();
    let mut sheets: Vec<XlsxSheet> = Vec::with_capacity(sheets_meta.len().min(8));
    for (name, range) in sheets_meta.into_iter().take(8) {
        let mut rows_iter = range.rows();
        let header: Vec<String> = rows_iter
            .next()
            .map(|r| r.iter().map(cell_to_string).collect())
            .unwrap_or_default();
        let preview_rows: Vec<Vec<String>> = rows_iter
            .take(200)
            .map(|r| r.iter().map(cell_to_string).collect())
            .collect();
        sheets.push(XlsxSheet {
            name,
            header,
            preview_rows,
            total_rows_hint: Some(range.height()),
            total_cols_hint: Some(range.width()),
            preview_formulas: None,
            merged_cells: None,
            frozen_panes: None,
            column_widths: None,
        });
    }
    Ok(Document::Xlsx {
        sheets,
        byte_len: bytes.len(),
    })
}

pub fn parse_xls_binary(bytes: &[u8]) -> Result<Document, Error> {
    use viewit_core_types::Document;

    let mut workbook = Xls::<Cursor<Vec<u8>>>::new(Cursor::new(bytes.to_vec()))
        .map_err(|e| Error::Parse(format!("calamine xls: {e}")))?;
    let sheets_meta = workbook.worksheets();
    let mut sheets: Vec<XlsxSheet> = Vec::with_capacity(sheets_meta.len().min(8));
    for (name, range) in sheets_meta.into_iter().take(8) {
        let mut rows_iter = range.rows();
        let header: Vec<String> = rows_iter
            .next()
            .map(|r| r.iter().map(cell_to_string).collect())
            .unwrap_or_default();
        let preview_rows: Vec<Vec<String>> = rows_iter
            .take(200)
            .map(|r| r.iter().map(cell_to_string).collect())
            .collect();
        sheets.push(XlsxSheet {
            name,
            header,
            preview_rows,
            total_rows_hint: Some(range.height()),
            total_cols_hint: Some(range.width()),
            preview_formulas: None,
            merged_cells: None,
            frozen_panes: None,
            column_widths: None,
        });
    }
    Ok(Document::Xlsx {
        sheets,
        byte_len: bytes.len(),
    })
}

fn cell_to_string(data: &Data) -> String {
    match data {
        Data::Empty => String::new(),
        Data::String(s) => s.clone(),
        Data::Float(v) => {
            if v.fract() == 0.0 && v.abs() < 1e15 {
                format!("{v:.0}")
            } else {
                v.to_string()
            }
        }
        Data::Int(v) => v.to_string(),
        Data::Bool(v) => v.to_string(),
        Data::DateTime(v) => v.to_string(),
        Data::DateTimeIso(v) => v.clone(),
        Data::DurationIso(v) => v.clone(),
        Data::Error(v) => format!("{v:?}"),
    }
}

/// Map each sheet name to its worksheet part path (`xl/worksheets/sheetN.xml`)
/// by walking workbook.xml → rels → sheet rel.
fn xlsx_worksheet_paths(bytes: &[u8]) -> Result<HashMap<String, String>, Error> {
    let workbook = zip_entry_string(bytes, "xl/workbook.xml")?;
    let workbook_rels = zip_entry_string(bytes, "xl/_rels/workbook.xml.rels")?;
    let rels = parse_relationships(&workbook_rels)?;
    let mut paths = HashMap::new();
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

fn parse_workbook_sheets(xml: &str) -> Result<Vec<(String, String)>, Error> {
    use quick_xml::events::Event;
    use quick_xml::name::QName;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut sheets = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if e.name() == QName(b"sheet") => {
                let name = attr_value(&e, b"name").unwrap_or_default();
                let rel_id = attr_value(&e, b"r:id")
                    .ok_or_else(|| Error::Parse("sheet missing r:id".into()))?;
                sheets.push((name, rel_id));
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(Error::Parse(format!("workbook parse: {e}"))),
            _ => {}
        }
    }
    Ok(sheets)
}

fn parse_relationships(xml: &str) -> Result<HashMap<String, String>, Error> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut rels = HashMap::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if e.name().as_ref() == b"Relationship" => {
                if let (Some(id), Some(target)) = (attr_value(&e, b"Id"), attr_value(&e, b"Target"))
                {
                    rels.insert(id, target);
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(Error::Parse(format!("relationships parse: {e}"))),
            _ => {}
        }
    }
    Ok(rels)
}

fn parse_sheet_merged_cells(xml: &str) -> Vec<String> {
    use quick_xml::events::Event;
    use quick_xml::name::QName;
    use quick_xml::Reader;

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

fn parse_sheet_frozen_pane(xml: &str) -> Option<XlsxFrozenPanes> {
    use quick_xml::events::Event;
    use quick_xml::name::QName;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut found = false;
    let mut x_split = 0u32;
    let mut y_split = 0u32;
    let mut active_pane = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Empty(e)) if e.name() == QName(b"pane") => {
                found = true;
                x_split = attr_value(&e, b"xSplit")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
                y_split = attr_value(&e, b"ySplit")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
                active_pane = attr_value(&e, b"activePane").unwrap_or_default();
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    if found && (x_split > 0 || y_split > 0) {
        Some(XlsxFrozenPanes {
            x_split,
            y_split,
            active_pane,
        })
    } else {
        None
    }
}

fn parse_sheet_col_widths(xml: &str) -> Vec<Option<f64>> {
    use quick_xml::events::Event;
    use quick_xml::name::QName;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut widths: Vec<Option<f64>> = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Empty(e)) if e.name() == QName(b"col") => {
                let Some(width) = attr_value(&e, b"width").and_then(|v| v.parse::<f64>().ok())
                else {
                    continue;
                };
                let min = attr_value(&e, b"min")
                    .and_then(|v| v.parse::<usize>().ok())
                    .unwrap_or(0);
                let max = attr_value(&e, b"max")
                    .and_then(|v| v.parse::<usize>().ok())
                    .unwrap_or(min);
                let min = min.saturating_sub(1);
                let max = max.saturating_sub(1);
                for i in min..=max.min(1024 * 4) {
                    if widths.len() <= i {
                        widths.resize(i + 1, None);
                    }
                    widths[i] = Some(width);
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    widths
}

fn parse_sheet_formulas(xml: &str) -> HashMap<String, String> {
    use quick_xml::events::Event;
    use quick_xml::name::QName;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut map = HashMap::new();
    let mut cell_ref: Option<String> = None;
    let mut in_f = false;
    let mut f_text = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if e.name() == QName(b"c") => {
                cell_ref = attr_value(&e, b"r");
                in_f = false;
                f_text.clear();
            }
            Ok(Event::Start(e)) if e.name() == QName(b"f") => in_f = true,
            Ok(Event::Text(t)) if in_f => {
                if let Ok(s) = t.unescape() {
                    f_text.push_str(&s);
                }
            }
            Ok(Event::End(e)) if e.name() == QName(b"f") => in_f = false,
            Ok(Event::End(e)) if e.name() == QName(b"c") => {
                if let Some(r) = cell_ref.take() {
                    let formula = f_text.trim().to_string();
                    if !formula.is_empty() {
                        map.insert(r, formula);
                    }
                }
                in_f = false;
                f_text.clear();
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    map
}

/// Align a cell-ref → formula map to `preview_rows` (excel row 1 = header).
fn align_formulas(
    header_cols: usize,
    preview_rows: &[Vec<String>],
    formulas: &HashMap<String, String>,
) -> Vec<Vec<String>> {
    let cols = preview_rows
        .iter()
        .map(Vec::len)
        .max()
        .unwrap_or(0)
        .max(header_cols);
    preview_rows
        .iter()
        .enumerate()
        .map(|(pr, _row)| {
            (0..cols)
                .map(|c| {
                    let expr = column_letter(c);
                    let row_no = pr + 2; // header is row 1
                    formulas
                        .get(&format!("{expr}{row_no}"))
                        .cloned()
                        .unwrap_or_default()
                })
                .collect()
        })
        .collect()
}

fn column_letter(index: usize) -> String {
    let mut n = index + 1;
    let mut label = String::new();
    while n > 0 {
        let rem = (n - 1) % 26;
        label.insert(0, (b'A' + rem as u8) as char);
        n = (n - 1) / 26;
    }
    label
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

fn zip_entry_string(bytes: &[u8], name: &str) -> Result<String, Error> {
    use std::io::Read;
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| Error::Parse(format!("zip open: {e}")))?;
    let mut file = archive
        .by_name(name)
        .map_err(|e| Error::Parse(format!("{name}: {e}")))?;
    let mut xml = String::new();
    file.read_to_string(&mut xml)
        .map_err(|e| Error::Parse(format!("{name} read: {e}")))?;
    Ok(xml)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_xlsx(sheet_xml: &str, workbook_xml: &str, rels_xml: &str) -> Vec<u8> {
        use std::io::Write;
        let content_types = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
<Default Extension="xml" ContentType="application/xml"/>
<Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
<Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
</Types>"#;
        let root_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"#;
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));
            let opts = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            zip.start_file("[Content_Types].xml", opts).unwrap();
            zip.write_all(content_types.as_bytes()).unwrap();
            zip.start_file("_rels/.rels", opts).unwrap();
            zip.write_all(root_rels.as_bytes()).unwrap();
            zip.start_file("xl/workbook.xml", opts).unwrap();
            zip.write_all(workbook_xml.as_bytes()).unwrap();
            zip.start_file("xl/_rels/workbook.xml.rels", opts).unwrap();
            zip.write_all(rels_xml.as_bytes()).unwrap();
            zip.start_file("xl/worksheets/sheet1.xml", opts).unwrap();
            zip.write_all(sheet_xml.as_bytes()).unwrap();
            zip.finish().unwrap();
        }
        buf
    }

    const WORKBOOK: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets><sheet name="Sheet1" sheetId="1" r:id="rId1"/></sheets>
</workbook>"#;

    const RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
</Relationships>"#;

    #[test]
    fn xlsx_enriches_merges_frozen_widths_formulas() {
        let sheet = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <cols><col min="1" max="1" width="22"/></cols>
  <mergeCells count="1"><mergeCell ref="A1:A2"/></mergeCells>
  <sheetViews><sheetView><pane ySplit="1" topLeftCell="A2" activePane="bottomLeft" state="frozen"/></sheetView></sheetViews>
  <sheetData>
    <row r="1"><c r="A1" t="inlineStr"><is><t>Header</t></is></c></row>
    <row r="2"><c r="A2"><v>10</v></c></row>
    <row r="3"><c r="A3"><f>SUM(A2:A2)</f><v>10</v></c></row>
  </sheetData>
</worksheet>"#;

        let bytes = build_xlsx(sheet, WORKBOOK, RELS);
        let doc = parse_xlsx(&bytes).unwrap();
        let doc = match doc {
            Document::Xlsx { sheets, .. } => sheets,
            _ => panic!("expected Xlsx"),
        };
        let s = &doc[0];
        assert_eq!(s.name, "Sheet1");
        assert_eq!(s.merged_cells.as_deref(), Some(&["A1:A2".to_string()][..]));
        assert_eq!(s.frozen_panes.as_ref().unwrap().y_split, 1);
        assert_eq!(s.column_widths.as_deref(), Some(&[Some(22.0)][..]));
        // preview_rows[1] is excel row 3 → A3 formula
        let f = s.preview_formulas.as_ref().unwrap();
        assert_eq!(f.len(), 2);
        assert_eq!(f[1][0], "SUM(A2:A2)");
    }

    #[test]
    fn xlsx_no_enrichment_parts_stays_none() {
        // Minimal sheet without merges/pane/cols/formulas.
        let sheet = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData>
    <row r="1"><c r="A1" t="inlineStr"><is><t>Header</t></is></c></row>
    <row r="2"><c r="A2"><v>42</v></c></row>
  </sheetData>
</worksheet>"#;
        let bytes = build_xlsx(sheet, WORKBOOK, RELS);
        let doc = parse_xlsx(&bytes).unwrap();
        let Document::Xlsx { sheets, .. } = doc else {
            panic!("expected Xlsx")
        };
        assert!(sheets[0].merged_cells.is_none());
        assert!(sheets[0].frozen_panes.is_none());
        assert!(sheets[0].column_widths.is_none());
        assert!(sheets[0].preview_formulas.is_none());
    }
}
