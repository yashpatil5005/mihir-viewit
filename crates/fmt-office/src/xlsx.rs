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
use viewit_core_types::{Document, Error, XlsxCellStyle, XlsxFrozenPanes, XlsxSheet};

// Include city: not used as a global here; keep module small.

pub fn parse_xlsx(bytes: &[u8]) -> Result<Document, Error> {
    use viewit_core_types::Document;

    let mut workbook = Xlsx::<Cursor<Vec<u8>>>::new(Cursor::new(bytes.to_vec()))
        .map_err(|e| Error::Parse(format!("calamine xlsx: {e}")))?;
    let sheets_meta = workbook.worksheets();
    let paths = xlsx_worksheet_paths(bytes).unwrap_or_default();
    let theme_colors = zip_entry_string(bytes, "xl/theme/theme1.xml")
        .ok()
        .map(|xml| parse_theme_colors(&xml))
        .unwrap_or_default();
    let style_table = zip_entry_string(bytes, "xl/styles.xml")
        .ok()
        .map(|xml| parse_xlsx_styles(&xml, &theme_colors));

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
        let mut cell_styles: Option<Vec<Vec<Option<XlsxCellStyle>>>> = None;
        let mut row_heights: Option<Vec<Option<f64>>> = None;

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
                let (start_row, start_col) = range
                    .start()
                    .map(|(row, col)| (row as usize + 1, col as usize))
                    .unwrap_or((1, 0));
                if !formulas.is_empty() {
                    preview_formulas = Some(align_formulas(
                        header.len(),
                        &preview_rows,
                        &formulas,
                        start_row,
                        start_col,
                    ));
                }
                let heights = parse_sheet_row_heights(&xml, start_row, preview_rows.len() + 1);
                if heights.iter().any(Option::is_some) {
                    row_heights = Some(heights);
                }
                if let Some(style_table) = &style_table {
                    let style_refs = parse_sheet_style_refs(&xml);
                    let aligned = align_cell_styles(
                        &header,
                        &preview_rows,
                        &style_refs,
                        style_table,
                        start_row,
                        start_col,
                    );
                    if aligned.iter().any(|row| row.iter().any(Option::is_some)) {
                        cell_styles = Some(aligned);
                    }
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
            cell_styles,
            row_heights,
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
            cell_styles: None,
            row_heights: None,
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
            cell_styles: None,
            row_heights: None,
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

fn parse_sheet_row_heights(
    xml: &str,
    first_emitted_row: usize,
    emitted_rows: usize,
) -> Vec<Option<f64>> {
    use quick_xml::events::Event;
    use quick_xml::name::QName;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    let mut heights = vec![None; emitted_rows];
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if e.name() == QName(b"row") => {
                let Some(row) = attr_value(&e, b"r").and_then(|v| v.parse::<usize>().ok()) else {
                    continue;
                };
                if row >= first_emitted_row && row < first_emitted_row + emitted_rows {
                    heights[row - first_emitted_row] =
                        attr_value(&e, b"ht").and_then(|v| v.parse().ok());
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    heights
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

fn parse_sheet_style_refs(xml: &str) -> HashMap<String, usize> {
    use quick_xml::events::Event;
    use quick_xml::name::QName;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    let mut styles = HashMap::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if e.name() == QName(b"c") => {
                if let (Some(cell), Some(style)) = (
                    attr_value(&e, b"r"),
                    attr_value(&e, b"s").and_then(|v| v.parse().ok()),
                ) {
                    styles.insert(cell, style);
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    styles
}

#[derive(Default)]
struct BorderStyle {
    top: Option<String>,
    right: Option<String>,
    bottom: Option<String>,
    left: Option<String>,
    color: Option<String>,
}

#[derive(Default)]
struct CellXf {
    font_id: usize,
    fill_id: usize,
    border_id: usize,
    num_fmt_id: u32,
    horizontal_alignment: Option<String>,
    vertical_alignment: Option<String>,
    wrap_text: bool,
}

fn parse_xlsx_styles(xml: &str, theme_colors: &[String]) -> Vec<Option<XlsxCellStyle>> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    let mut section = String::new();
    let mut fonts = Vec::<XlsxCellStyle>::new();
    let mut fills = Vec::<Option<String>>::new();
    let mut borders = Vec::<BorderStyle>::new();
    let mut xfs = Vec::<CellXf>::new();
    let mut number_formats = HashMap::<u32, String>::new();
    let mut current = XlsxCellStyle::default();
    let mut current_border = BorderStyle::default();
    let mut border_side: Option<String> = None;
    let mut current_xf: Option<CellXf> = None;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => match e.name().local_name().as_ref() {
                b"fonts" | b"fills" | b"borders" | b"cellXfs" | b"numFmts" => {
                    section = String::from_utf8_lossy(e.name().local_name().as_ref()).to_string();
                }
                b"font" if section == "fonts" => current = XlsxCellStyle::default(),
                b"fill" if section == "fills" => current = XlsxCellStyle::default(),
                b"border" if section == "borders" => current_border = BorderStyle::default(),
                b"left" | b"right" | b"top" | b"bottom" if section == "borders" => {
                    border_side =
                        Some(String::from_utf8_lossy(e.name().local_name().as_ref()).into());
                    set_border_side(
                        &mut current_border,
                        border_side.as_deref().unwrap_or_default(),
                        attr_value(&e, b"style").as_deref(),
                        None,
                    );
                }
                b"b" if section == "fonts" => current.bold = true,
                b"i" if section == "fonts" => current.italic = true,
                b"color" if section == "fonts" => {
                    current.font_color = ooxml_color(&e, theme_colors)
                }
                b"fgColor" if section == "fills" => {
                    current.fill_color = ooxml_color(&e, theme_colors)
                }
                b"color" if section == "borders" => {
                    if let Some(side) = &border_side {
                        set_border_side(
                            &mut current_border,
                            side,
                            None,
                            ooxml_color(&e, theme_colors),
                        );
                    }
                }
                b"xf" if section == "cellXfs" => current_xf = Some(cell_xf(&e)),
                _ => {}
            },
            Ok(Event::Empty(e)) => match e.name().local_name().as_ref() {
                b"font" if section == "fonts" => fonts.push(XlsxCellStyle::default()),
                b"fill" if section == "fills" => fills.push(None),
                b"border" if section == "borders" => borders.push(BorderStyle::default()),
                b"b" if section == "fonts" => current.bold = true,
                b"i" if section == "fonts" => current.italic = true,
                b"color" if section == "fonts" => {
                    current.font_color = ooxml_color(&e, theme_colors)
                }
                b"fgColor" if section == "fills" => {
                    current.fill_color = ooxml_color(&e, theme_colors)
                }
                b"left" | b"right" | b"top" | b"bottom" if section == "borders" => {
                    let local_name = e.name().local_name();
                    let side = String::from_utf8_lossy(local_name.as_ref());
                    set_border_side(
                        &mut current_border,
                        &side,
                        attr_value(&e, b"style").as_deref(),
                        None,
                    );
                }
                b"color" if section == "borders" => {
                    if let Some(side) = &border_side {
                        set_border_side(
                            &mut current_border,
                            side,
                            None,
                            ooxml_color(&e, theme_colors),
                        );
                    }
                }
                b"numFmt" if section == "numFmts" => {
                    if let (Some(id), Some(code)) = (
                        attr_value(&e, b"numFmtId").and_then(|v| v.parse().ok()),
                        attr_value(&e, b"formatCode"),
                    ) {
                        number_formats.insert(id, code);
                    }
                }
                b"xf" if section == "cellXfs" => xfs.push(cell_xf(&e)),
                b"alignment" if section == "cellXfs" => {
                    if let Some(xf) = current_xf.as_mut() {
                        apply_alignment(xf, &e);
                    }
                }
                _ => {}
            },
            Ok(Event::End(e)) => match e.name().local_name().as_ref() {
                b"font" if section == "fonts" => fonts.push(std::mem::take(&mut current)),
                b"fill" if section == "fills" => fills.push(current.fill_color.take()),
                b"left" | b"right" | b"top" | b"bottom" if section == "borders" => {
                    border_side = None
                }
                b"border" if section == "borders" => {
                    borders.push(std::mem::take(&mut current_border))
                }
                b"xf" if section == "cellXfs" => {
                    if let Some(xf) = current_xf.take() {
                        xfs.push(xf);
                    }
                }
                b"fonts" | b"fills" | b"borders" | b"cellXfs" | b"numFmts" => section.clear(),
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }

    xfs.into_iter()
        .map(|xf| {
            let font = fonts.get(xf.font_id).cloned().unwrap_or_default();
            let border = borders.get(xf.border_id);
            let style = XlsxCellStyle {
                fill_color: fills.get(xf.fill_id).cloned().flatten(),
                font_color: font.font_color,
                bold: font.bold,
                italic: font.italic,
                border_color: border.and_then(|b| b.color.clone()),
                border_top: border.and_then(|b| b.top.clone()),
                border_right: border.and_then(|b| b.right.clone()),
                border_bottom: border.and_then(|b| b.bottom.clone()),
                border_left: border.and_then(|b| b.left.clone()),
                number_format: number_formats
                    .get(&xf.num_fmt_id)
                    .cloned()
                    .or_else(|| builtin_number_format(xf.num_fmt_id).map(str::to_string)),
                horizontal_alignment: xf.horizontal_alignment,
                vertical_alignment: xf.vertical_alignment,
                wrap_text: xf.wrap_text,
            };
            if style.fill_color.is_some()
                || style.font_color.is_some()
                || style.bold
                || style.italic
                || style.border_color.is_some()
                || style.border_top.is_some()
                || style.border_right.is_some()
                || style.border_bottom.is_some()
                || style.border_left.is_some()
                || style.number_format.is_some()
                || style.horizontal_alignment.is_some()
                || style.vertical_alignment.is_some()
                || style.wrap_text
            {
                Some(style)
            } else {
                None
            }
        })
        .collect()
}

fn cell_xf(e: &quick_xml::events::BytesStart<'_>) -> CellXf {
    CellXf {
        font_id: attr_value(e, b"fontId")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0),
        fill_id: attr_value(e, b"fillId")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0),
        border_id: attr_value(e, b"borderId")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0),
        num_fmt_id: attr_value(e, b"numFmtId")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0),
        ..Default::default()
    }
}

fn apply_alignment(xf: &mut CellXf, e: &quick_xml::events::BytesStart<'_>) {
    xf.horizontal_alignment = attr_value(e, b"horizontal");
    xf.vertical_alignment = attr_value(e, b"vertical");
    xf.wrap_text = attr_value(e, b"wrapText").is_some_and(|v| v == "1" || v == "true");
}

fn set_border_side(
    border: &mut BorderStyle,
    side: &str,
    style: Option<&str>,
    color: Option<String>,
) {
    let slot = match side {
        "top" => &mut border.top,
        "right" => &mut border.right,
        "bottom" => &mut border.bottom,
        "left" => &mut border.left,
        _ => return,
    };
    if let Some(style) = style {
        *slot = border_css(style, None);
    }
    if let Some(color) = color {
        border.color.get_or_insert_with(|| color.clone());
        if let Some(value) = slot {
            let style = value
                .split_whitespace()
                .take(2)
                .collect::<Vec<_>>()
                .join(" ");
            *value = format!("{style} {color}");
        }
    }
}

fn border_css(style: &str, color: Option<&str>) -> Option<String> {
    let (width, line) = match style {
        "hair" => ("1px", "dotted"),
        "dotted" => ("1px", "dotted"),
        "dashDot" | "dashDotDot" | "dashed" | "slantDashDot" => ("1px", "dashed"),
        "mediumDashDot" | "mediumDashDotDot" | "mediumDashed" => ("2px", "dashed"),
        "medium" => ("2px", "solid"),
        "thick" | "double" => ("3px", if style == "double" { "double" } else { "solid" }),
        "thin" => ("1px", "solid"),
        _ => return None,
    };
    Some(format!(
        "{width} {line} {}",
        color.unwrap_or("currentColor")
    ))
}

fn parse_theme_colors(xml: &str) -> Vec<String> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    let mut in_scheme = false;
    let mut colors = vec![String::new(); 12];
    let mut slot: Option<usize> = None;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if e.name().local_name().as_ref() == b"clrScheme" => {
                in_scheme = true
            }
            Ok(Event::Start(e)) if in_scheme => {
                slot = theme_color_slot(e.name().local_name().as_ref());
            }
            Ok(Event::Empty(e)) if in_scheme && e.name().local_name().as_ref() == b"srgbClr" => {
                if let (Some(index), Some(value)) = (slot, attr_value(&e, b"val").and_then(css_rgb))
                {
                    colors[index] = value;
                }
            }
            Ok(Event::Empty(e)) if in_scheme && e.name().local_name().as_ref() == b"sysClr" => {
                if let (Some(index), Some(value)) =
                    (slot, attr_value(&e, b"lastClr").and_then(css_rgb))
                {
                    colors[index] = value;
                }
            }
            Ok(Event::End(e)) if theme_color_slot(e.name().local_name().as_ref()).is_some() => {
                slot = None;
            }
            Ok(Event::End(e)) if e.name().local_name().as_ref() == b"clrScheme" => break,
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    colors
}

fn theme_color_slot(name: &[u8]) -> Option<usize> {
    [
        b"dk1".as_slice(),
        b"lt1",
        b"dk2",
        b"lt2",
        b"accent1",
        b"accent2",
        b"accent3",
        b"accent4",
        b"accent5",
        b"accent6",
        b"hlink",
        b"folHlink",
    ]
    .iter()
    .position(|candidate| *candidate == name)
}

fn ooxml_color(e: &quick_xml::events::BytesStart<'_>, theme_colors: &[String]) -> Option<String> {
    let color = if let Some(rgb) = attr_value(e, b"rgb").and_then(css_rgb) {
        rgb
    } else if let Some(theme) = attr_value(e, b"theme").and_then(|v| v.parse::<usize>().ok()) {
        theme_colors.get(theme)?.clone()
    } else if let Some(index) = attr_value(e, b"indexed").and_then(|v| v.parse::<usize>().ok()) {
        indexed_color(index)?.to_string()
    } else {
        return None;
    };
    let tint = attr_value(e, b"tint")
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(0.0);
    Some(apply_tint(&color, tint))
}

fn css_rgb(rgb: String) -> Option<String> {
    let rgb = if rgb.len() == 8 { &rgb[2..] } else { &rgb };
    (rgb.len() == 6 && rgb.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| format!("#{}", rgb.to_ascii_uppercase()))
}

fn apply_tint(color: &str, tint: f64) -> String {
    if tint == 0.0 || color.len() != 7 {
        return color.to_string();
    }
    let channel = |offset| {
        let value = u8::from_str_radix(&color[offset..offset + 2], 16).unwrap_or(0) as f64;
        let adjusted = if tint < 0.0 {
            value * (1.0 + tint)
        } else {
            value + (255.0 - value) * tint
        };
        adjusted.round().clamp(0.0, 255.0) as u8
    };
    format!("#{:02X}{:02X}{:02X}", channel(1), channel(3), channel(5))
}

fn indexed_color(index: usize) -> Option<&'static str> {
    const COLORS: [&str; 64] = [
        "#000000", "#FFFFFF", "#FF0000", "#00FF00", "#0000FF", "#FFFF00", "#FF00FF", "#00FFFF",
        "#000000", "#FFFFFF", "#FF0000", "#00FF00", "#0000FF", "#FFFF00", "#FF00FF", "#00FFFF",
        "#800000", "#008000", "#000080", "#808000", "#800080", "#008080", "#C0C0C0", "#808080",
        "#9999FF", "#993366", "#FFFFCC", "#CCFFFF", "#660066", "#FF8080", "#0066CC", "#CCCCFF",
        "#000080", "#FF00FF", "#FFFF00", "#00FFFF", "#800080", "#800000", "#008080", "#0000FF",
        "#00CCFF", "#CCFFFF", "#CCFFCC", "#FFFF99", "#99CCFF", "#FF99CC", "#CC99FF", "#FFCC99",
        "#3366FF", "#33CCCC", "#99CC00", "#FFCC00", "#FF9900", "#FF6600", "#666699", "#969696",
        "#003366", "#339966", "#003300", "#333300", "#993300", "#993366", "#333399", "#333333",
    ];
    COLORS.get(index).copied()
}

fn builtin_number_format(id: u32) -> Option<&'static str> {
    match id {
        1 => Some("0"),
        2 => Some("0.00"),
        3 => Some("#,##0"),
        4 => Some("#,##0.00"),
        9 => Some("0%"),
        10 => Some("0.00%"),
        11 => Some("0.00E+00"),
        12 => Some("# ?/?"),
        14 => Some("m/d/yy"),
        15 => Some("d-mmm-yy"),
        16 => Some("d-mmm"),
        17 => Some("mmm-yy"),
        18 => Some("h:mm AM/PM"),
        19 => Some("h:mm:ss AM/PM"),
        20 => Some("h:mm"),
        21 => Some("h:mm:ss"),
        22 => Some("m/d/yy h:mm"),
        37 => Some("#,##0 ;(#,##0)"),
        38 => Some("#,##0 ;[Red](#,##0)"),
        39 => Some("#,##0.00;(#,##0.00)"),
        40 => Some("#,##0.00;[Red](#,##0.00)"),
        41 => Some("_(* #,##0_);_(* (#,##0);_(* \"-\"_);_(@_)"),
        42 => Some("_($* #,##0_);_($* (#,##0);_($* \"-\"_);_(@_)"),
        43 => Some("_(* #,##0.00_);_(* (#,##0.00);_(* \"-\"??_);_(@_)"),
        44 => Some("_($* #,##0.00_);_($* (#,##0.00);_($* \"-\"??_);_(@_)"),
        45 => Some("mm:ss"),
        46 => Some("[h]:mm:ss"),
        47 => Some("mmss.0"),
        48 => Some("##0.0E+0"),
        49 => Some("@"),
        _ => None,
    }
}

fn align_cell_styles(
    header: &[String],
    preview_rows: &[Vec<String>],
    refs: &HashMap<String, usize>,
    styles: &[Option<XlsxCellStyle>],
    start_row: usize,
    start_col: usize,
) -> Vec<Vec<Option<XlsxCellStyle>>> {
    let cols = preview_rows
        .iter()
        .map(Vec::len)
        .max()
        .unwrap_or(0)
        .max(header.len());
    (0..=preview_rows.len())
        .map(|row| {
            (0..cols)
                .map(|col| {
                    refs.get(&format!(
                        "{}{}",
                        column_letter(start_col + col),
                        start_row + row
                    ))
                        .and_then(|id| styles.get(*id))
                        .cloned()
                        .flatten()
                })
                .collect()
        })
        .collect()
}

/// Align a cell-ref → formula map to `preview_rows` (excel row 1 = header).
fn align_formulas(
    header_cols: usize,
    preview_rows: &[Vec<String>],
    formulas: &HashMap<String, String>,
    start_row: usize,
    start_col: usize,
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
                    let expr = column_letter(start_col + c);
                    let row_no = start_row + pr + 1;
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

    fn build_xlsx_with_styles(
        sheet_xml: &str,
        workbook_xml: &str,
        rels_xml: &str,
        styles_xml: Option<&str>,
    ) -> Vec<u8> {
        build_xlsx_with_parts(sheet_xml, workbook_xml, rels_xml, styles_xml, None)
    }

    fn build_xlsx_with_parts(
        sheet_xml: &str,
        workbook_xml: &str,
        rels_xml: &str,
        styles_xml: Option<&str>,
        theme_xml: Option<&str>,
    ) -> Vec<u8> {
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
            if let Some(styles) = styles_xml {
                zip.start_file("xl/styles.xml", opts).unwrap();
                zip.write_all(styles.as_bytes()).unwrap();
            }
            if let Some(theme) = theme_xml {
                zip.start_file("xl/theme/theme1.xml", opts).unwrap();
                zip.write_all(theme.as_bytes()).unwrap();
            }
            zip.finish().unwrap();
        }
        buf
    }

    fn build_xlsx(sheet_xml: &str, workbook_xml: &str, rels_xml: &str) -> Vec<u8> {
        build_xlsx_with_styles(sheet_xml, workbook_xml, rels_xml, None)
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
        assert!(sheets[0].cell_styles.is_none());
    }

    #[test]
    fn xlsx_extracts_sparse_cell_styles() {
        let sheet = r#"<?xml version="1.0"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>
<row r="1"><c r="A1" s="1" t="inlineStr"><is><t>Header</t></is></c></row>
<row r="2"><c r="A2" s="2"><v>42</v></c></row>
</sheetData></worksheet>"#;
        let styles = r#"<?xml version="1.0"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
<fonts count="2"><font/><font><b/><color rgb="FFFF0000"/></font></fonts>
<fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="solid"><fgColor rgb="FF00FF00"/></patternFill></fill></fills>
<borders count="2"><border/><border><left><color rgb="FF0000FF"/></left></border></borders>
<cellXfs count="3"><xf fontId="0" fillId="0" borderId="0"/><xf fontId="1" fillId="0" borderId="0"/><xf fontId="0" fillId="1" borderId="1"/></cellXfs>
</styleSheet>"#;
        let bytes = build_xlsx_with_styles(sheet, WORKBOOK, RELS, Some(styles));
        let Document::Xlsx { sheets, .. } = parse_xlsx(&bytes).unwrap() else {
            panic!("expected Xlsx")
        };
        let styles = sheets[0].cell_styles.as_ref().unwrap();
        assert!(styles[0][0].as_ref().unwrap().bold);
        assert_eq!(
            styles[0][0].as_ref().unwrap().font_color.as_deref(),
            Some("#FF0000")
        );
        assert_eq!(
            styles[1][0].as_ref().unwrap().fill_color.as_deref(),
            Some("#00FF00")
        );
        assert_eq!(
            styles[1][0].as_ref().unwrap().border_color.as_deref(),
            Some("#0000FF")
        );
    }

    #[test]
    fn xlsx_aligns_enrichment_to_non_a1_used_range() {
        let sheet = r#"<?xml version="1.0"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="5" ht="24"><c r="C5" s="1" t="inlineStr"><is><t>Header</t></is></c></row><row r="6"><c r="C6" s="1"><f>SUM(C5:C5)</f><v>1</v></c></row></sheetData></worksheet>"#;
        let styles = r#"<?xml version="1.0"?><styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><fonts count="2"><font/><font><b/></font></fonts><fills count="1"><fill/></fills><borders count="1"><border/></borders><cellXfs count="2"><xf/><xf fontId="1"/></cellXfs></styleSheet>"#;
        let bytes = build_xlsx_with_styles(sheet, WORKBOOK, RELS, Some(styles));
        let Document::Xlsx { sheets, .. } = parse_xlsx(&bytes).unwrap() else {
            panic!("expected Xlsx")
        };
        let sheet = &sheets[0];
        assert_eq!(sheet.row_heights.as_deref(), Some(&[Some(24.0), None][..]));
        assert!(sheet.cell_styles.as_ref().unwrap()[0][0].as_ref().unwrap().bold);
        assert_eq!(sheet.preview_formulas.as_ref().unwrap()[0][0], "SUM(C5:C5)");
    }

    #[test]
    fn xlsx_extracts_round_four_styles_and_row_heights() {
        let sheet = r#"<?xml version="1.0"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>
<row r="1" ht="24" customHeight="1"><c r="A1" s="1" t="inlineStr"><is><t>Merged</t></is></c><c r="B1"/></row>
<row r="2" ht="30" customHeight="1"><c r="A2"/><c r="B2" s="2"><v>42</v></c><c r="C2" s="3"><v>1</v></c></row>
</sheetData><mergeCells count="1"><mergeCell ref="A1:B1"/></mergeCells></worksheet>"#;
        let styles = r#"<?xml version="1.0"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
<numFmts count="1"><numFmt numFmtId="164" formatCode="$#,##0.00"/></numFmts>
<fonts count="3"><font/><font><color theme="4" tint="0.5"/></font><font><color indexed="10"/></font></fonts>
<fills count="2"><fill/><fill><patternFill patternType="solid"><fgColor rgb="FF123456"/></patternFill></fill></fills>
<borders count="2"><border/><border><left style="thin"><color rgb="FFFF0000"/></left><right style="mediumDashed"><color theme="5"/></right><top style="double"><color indexed="12"/></top><bottom style="dotted"><color rgb="FF00FF00"/></bottom></border></borders>
<cellXfs count="4"><xf/><xf fontId="1" fillId="1" borderId="1" numFmtId="164"><alignment horizontal="center" vertical="center" wrapText="1"/></xf><xf fontId="2" numFmtId="14"/><xf numFmtId="4"/></cellXfs>
</styleSheet>"#;
        let theme = r#"<?xml version="1.0"?><a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:themeElements><a:clrScheme name="Office"><a:dk1><a:sysClr val="windowText" lastClr="000000"/></a:dk1><a:lt1><a:sysClr val="window" lastClr="FFFFFF"/></a:lt1><a:dk2><a:srgbClr val="111111"/></a:dk2><a:lt2><a:srgbClr val="EEEEEE"/></a:lt2><a:accent1><a:srgbClr val="336699"/></a:accent1><a:accent2><a:srgbClr val="ABCDEF"/></a:accent2></a:clrScheme></a:themeElements></a:theme>"#;
        let bytes = build_xlsx_with_parts(sheet, WORKBOOK, RELS, Some(styles), Some(theme));
        let Document::Xlsx { sheets, .. } = parse_xlsx(&bytes).unwrap() else {
            panic!("expected Xlsx")
        };
        let sheet = &sheets[0];
        assert_eq!(
            sheet.row_heights.as_deref(),
            Some(&[Some(24.0), Some(30.0)][..])
        );
        let grid = sheet.cell_styles.as_ref().unwrap();
        let merged_anchor = grid[0][0].as_ref().unwrap();
        assert_eq!(merged_anchor.fill_color.as_deref(), Some("#123456"));
        assert_eq!(merged_anchor.font_color.as_deref(), Some("#99B3CC"));
        assert_eq!(merged_anchor.number_format.as_deref(), Some("$#,##0.00"));
        assert_eq!(
            merged_anchor.horizontal_alignment.as_deref(),
            Some("center")
        );
        assert_eq!(merged_anchor.vertical_alignment.as_deref(), Some("center"));
        assert!(merged_anchor.wrap_text);
        assert_eq!(
            merged_anchor.border_left.as_deref(),
            Some("1px solid #FF0000")
        );
        assert_eq!(
            merged_anchor.border_right.as_deref(),
            Some("2px dashed #ABCDEF")
        );
        assert_eq!(
            merged_anchor.border_top.as_deref(),
            Some("3px double #0000FF")
        );
        assert_eq!(
            merged_anchor.border_bottom.as_deref(),
            Some("1px dotted #00FF00")
        );
        assert!(grid[0][1].is_none(), "covered merged cells stay sparse");
        assert_eq!(
            grid[1][1].as_ref().unwrap().font_color.as_deref(),
            Some("#FF0000")
        );
        assert_eq!(
            grid[1][1].as_ref().unwrap().number_format.as_deref(),
            Some("m/d/yy")
        );
        assert_eq!(
            grid[1][2].as_ref().unwrap().number_format.as_deref(),
            Some("#,##0.00")
        );
    }
}
