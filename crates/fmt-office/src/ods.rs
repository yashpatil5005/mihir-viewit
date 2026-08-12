//! ODS / OTS parser (OpenDocument Spreadsheet).
//!
//! Native quick-xml parser over the `content.xml` zip entry. Unlike the
//! legacy calamine reader (which flipped sheet order and dropped cell
//! placement/formulas), this preserves:
//!   - document sheet order (Sheet1 before MyLinks),
//!   - cell placement (blank `table:table-cell` gaps become empty strings),
//!   - formula sources (exposed per cell via `preview_formulas`).
//!
//! Falls back to calamine when `content.xml` is unavailable or yields no
//! sheets (e.g. an unusual ODS dialect our streaming reader can't parse).

use std::io::{Cursor, Read};
use viewit_core_types::{Document, Error, Format, XlsxSheet};
use zip::ZipArchive;

const MAX_PREVIEW_ROWS: usize = 1000;
const MAX_EMIT_COLS: usize = 512;
const MAX_SHEETS: usize = 32;

pub fn parse_ods(bytes: &[u8], format: Format, name: &str) -> Result<Document, Error> {
    let cursor = Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor).map_err(|e| Error::Parse(format!("zip: {}", e)))?;
    let mut content_xml = String::new();
    match archive.by_name("content.xml") {
        Ok(mut f) => {
            f.read_to_string(&mut content_xml)
                .map_err(|e| Error::Parse(format!("read content.xml: {}", e)))?;
        }
        Err(_) => return parse_ods_calamine(bytes, format, name),
    }

    let sheets = parse_ods_sheets(&content_xml);
    if sheets.is_empty() {
        return parse_ods_calamine(bytes, format, name);
    }

    Ok(Document::Xlsx {
        sheets,
        byte_len: bytes.len(),
    })
}

/// Final CellMonad — the display text plus optional formula (already stripped
/// of the `of:` prefix).
#[derive(Debug, Clone)]
struct CellVal {
    text: String,
    formula: Option<String>,
}

#[derive(Default)]
struct SheetAcc {
    name: String,
    rows: Vec<Vec<Option<CellVal>>>,
    used_cols_max: usize,
    used_rows_max: usize,
    content_cols_max: usize,
    content_rows_max: usize,
    row_has_content: bool,
    cur_abs_row: usize,
    row_cells: Vec<Option<CellVal>>,
    col_cursor: usize,
    row_repeat: usize,
    cell_repeat: usize,
    in_row: bool,
    in_cell: bool,
    in_annotation: bool,
    cell_text: String,
    cell_value: Option<String>,
    cell_value_type: Option<String>,
    cell_formula: Option<String>,
}

impl SheetAcc {
    fn begin_row(&mut self, e: &quick_xml::events::BytesStart<'_>) {
        self.in_row = true;
        self.row_cells.clear();
        self.col_cursor = 0;
        self.row_repeat = 1;
        for attr in e.attributes().flatten() {
            if attr.key.as_ref() == b"table:number-rows-repeated" {
                if let Ok(n) = String::from_utf8_lossy(attr.value.as_ref()).parse::<usize>() {
                    self.row_repeat = n.max(1);
                }
            }
        }
    }

    fn begin_cell(&mut self, e: &quick_xml::events::BytesStart<'_>) {
        self.in_cell = true;
        self.cell_text.clear();
        self.cell_value = None;
        self.cell_value_type = None;
        self.cell_formula = None;
        self.cell_repeat = 1;
        for attr in e.attributes().flatten() {
            let key = attr.key.as_ref();
            let val = String::from_utf8_lossy(attr.value.as_ref()).to_string();
            match key {
                b"table:number-columns-repeated" => {
                    if let Ok(n) = val.parse::<usize>() {
                        self.cell_repeat = n.max(1);
                    }
                }
                b"office:value-type" => self.cell_value_type = Some(val),
                b"office:value"
                | b"office:boolean-value"
                | b"office:date-value"
                | b"office:time-value"
                | b"office:string-value" => self.cell_value = Some(val),
                b"table:formula" => self.cell_formula = Some(strip_formula_prefix(&val)),
                _ => {}
            }
        }
    }

    fn end_cell(&mut self) {
        if !self.in_cell {
            return;
        }
        self.in_cell = false;

        let text = match self.cell_value_type.as_deref() {
            Some("string") => self
                .cell_value
                .clone()
                .or_else(|| non_empty(&self.cell_text))
                .unwrap_or_default(),
            Some("boolean") | Some("date") | Some("time") | Some("float") | Some("currency")
            | Some("percentage") => self.cell_value.clone().unwrap_or_default(),
            _ => self
                .cell_value
                .clone()
                .or_else(|| non_empty(&self.cell_text))
                .unwrap_or_default(),
        };
        let has_any = self.cell_formula.is_some() || !text.is_empty() || self.cell_value.is_some();

        let col_start = self.col_cursor;
        self.col_cursor += self.cell_repeat;
        if self.used_cols_max < self.col_cursor {
            self.used_cols_max = self.col_cursor;
        }
        if !has_any {
            return;
        }

        self.row_has_content = true;
        if self.content_cols_max < self.col_cursor {
            self.content_cols_max = self.col_cursor;
        }

        let limit = self.col_cursor.min(MAX_EMIT_COLS);
        if self.row_cells.len() < limit {
            self.row_cells.resize(limit, None);
        }
        for i in col_start.min(MAX_EMIT_COLS)..limit {
            self.row_cells[i] = Some(CellVal {
                text: text.clone(),
                formula: self.cell_formula.clone(),
            });
        }
    }

    fn end_row(&mut self) {
        self.in_row = false;
        if self.row_has_content {
            self.content_rows_max =
                std::cmp::max(self.content_rows_max, self.cur_abs_row + self.row_repeat);
            self.row_has_content = false;
        }
        let remaining_window = MAX_PREVIEW_ROWS.saturating_sub(self.cur_abs_row);
        let clone_count = self.row_repeat.min(remaining_window);
        for _ in 0..clone_count {
            self.rows.push(self.row_cells.clone());
        }
        self.cur_abs_row += self.row_repeat;
        if self.used_rows_max < self.cur_abs_row {
            self.used_rows_max = self.cur_abs_row;
        }
        self.row_cells.clear();
        self.col_cursor = 0;
    }

    fn begin_para(&mut self) {
        if !self.cell_text.is_empty() {
            self.cell_text.push('\n');
        }
    }
}

fn non_empty(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

/// `of:=SUM(...)` → `=SUM(...)`; requires the spreadsheet (not `table:formula`
/// style used inside `of:=`). Unknown prefixes are passed through unchanged.
fn strip_formula_prefix(f: &str) -> String {
    f.strip_prefix("of:")
        .map(|s| s.to_string())
        .unwrap_or_else(|| f.to_string())
}

fn pad_cells(r: &[Option<CellVal>], width: usize) -> Vec<String> {
    (0..width)
        .map(|i| {
            r.get(i)
                .and_then(|c| c.as_ref())
                .map(|c| c.text.clone())
                .unwrap_or_default()
        })
        .collect()
}

fn pad_formulas(r: &[Option<CellVal>], width: usize) -> Vec<String> {
    (0..width)
        .map(|i| {
            r.get(i)
                .and_then(|c| c.as_ref())
                .and_then(|c| c.formula.clone())
                .unwrap_or_default()
        })
        .collect()
}

fn push_sheet(a: SheetAcc, sheets: &mut Vec<XlsxSheet>) {
    if sheets.len() >= MAX_SHEETS {
        return;
    }
    let emit_cols = a.content_cols_max.clamp(1, MAX_EMIT_COLS);
    let trimmed_len = a.rows.len().min(a.content_rows_max);
    if trimmed_len == 0 {
        return;
    }
    let rows = &a.rows[..trimmed_len];
    let header = pad_cells(&rows[0], emit_cols);
    let preview_rows: Vec<Vec<String>> = rows.iter().map(|r| pad_cells(r, emit_cols)).collect();
    let preview_formulas: Vec<Vec<String>> =
        rows.iter().map(|r| pad_formulas(r, emit_cols)).collect();

    sheets.push(XlsxSheet {
        name: a.name,
        header,
        preview_rows,
        total_rows_hint: Some(a.content_rows_max),
        total_cols_hint: Some(a.content_cols_max),
        preview_formulas: Some(preview_formulas),
        merged_cells: None,
        frozen_panes: None,
        column_widths: None,
    });
}

fn parse_ods_sheets(xml: &str) -> Vec<XlsxSheet> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut sheets: Vec<XlsxSheet> = Vec::new();
    let mut acc: Option<SheetAcc> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => match e.name().as_ref() {
                b"table:table" => {
                    if let Some(a) = acc.take() {
                        push_sheet(a, &mut sheets);
                    }
                    let mut a = SheetAcc::default();
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"table:name" {
                            a.name = String::from_utf8_lossy(attr.value.as_ref()).to_string();
                        }
                    }
                    acc = Some(a);
                }
                b"table:table-row" => {
                    if let Some(a) = acc.as_mut() {
                        a.begin_row(&e);
                    }
                }
                b"table:table-cell" => {
                    if let Some(a) = acc.as_mut() {
                        a.begin_cell(&e);
                    }
                }
                b"text:p" | b"text:h" => {
                    if let Some(a) = acc.as_mut() {
                        a.begin_para();
                    }
                }
                b"text:line-break" => {
                    if let Some(a) = acc.as_mut() {
                        if a.in_cell {
                            a.cell_text.push('\n');
                        }
                    }
                }
                b"text:tab" => {
                    if let Some(a) = acc.as_mut() {
                        if a.in_cell {
                            a.cell_text.push('\t');
                        }
                    }
                }
                b"text:s" => {
                    if let Some(a) = acc.as_mut() {
                        if a.in_cell {
                            push_spaces(&mut a.cell_text, &e);
                        }
                    }
                }
                b"office:annotation" => {
                    if let Some(a) = acc.as_mut() {
                        a.in_annotation = true;
                    }
                }
                _ => {}
            },
            Ok(Event::Empty(e)) => match e.name().as_ref() {
                b"table:table-cell" => {
                    if let Some(a) = acc.as_mut() {
                        a.begin_cell(&e);
                        a.end_cell();
                    }
                }
                b"text:line-break" => {
                    if let Some(a) = acc.as_mut() {
                        if a.in_cell {
                            a.cell_text.push('\n');
                        }
                    }
                }
                b"text:tab" => {
                    if let Some(a) = acc.as_mut() {
                        if a.in_cell {
                            a.cell_text.push('\t');
                        }
                    }
                }
                b"text:s" => {
                    if let Some(a) = acc.as_mut() {
                        if a.in_cell {
                            push_spaces(&mut a.cell_text, &e);
                        }
                    }
                }
                _ => {}
            },
            Ok(Event::Text(t)) => {
                if let Some(a) = acc.as_mut() {
                    if a.in_cell && !a.in_annotation {
                        let s = t.unescape().unwrap_or_default();
                        a.cell_text.push_str(&s);
                    }
                }
            }
            Ok(Event::CData(c)) => {
                if let Some(a) = acc.as_mut() {
                    if a.in_cell && !a.in_annotation {
                        a.cell_text.push_str(&String::from_utf8_lossy(c.as_ref()));
                    }
                }
            }
            Ok(Event::End(e)) => match e.name().as_ref() {
                b"table:table-cell" => {
                    if let Some(a) = acc.as_mut() {
                        a.end_cell();
                    }
                }
                b"table:table-row" => {
                    if let Some(a) = acc.as_mut() {
                        a.end_row();
                    }
                }
                b"table:table" => {
                    if let Some(a) = acc.take() {
                        push_sheet(a, &mut sheets);
                    }
                }
                b"office:annotation" => {
                    if let Some(a) = acc.as_mut() {
                        a.in_annotation = false;
                    }
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    if let Some(a) = acc.take() {
        push_sheet(a, &mut sheets);
    }

    sheets
}

fn push_spaces(text: &mut String, e: &quick_xml::events::BytesStart<'_>) {
    let count = e
        .attributes()
        .flatten()
        .find(|attr| attr.key.as_ref() == b"text:c")
        .and_then(|attr| {
            String::from_utf8_lossy(attr.value.as_ref())
                .parse::<usize>()
                .ok()
        })
        .unwrap_or(1)
        .min(64);
    for _ in 0..count {
        text.push(' ');
    }
}

/// Legacy calamine reader, kept as a fallback (no formula/placement info).
fn parse_ods_calamine(bytes: &[u8], _format: Format, _name: &str) -> Result<Document, Error> {
    use calamine::Reader;

    let cursor = Cursor::new(bytes.to_vec());
    let mut workbook = calamine::Ods::<Cursor<Vec<u8>>>::new(cursor)
        .map_err(|e| Error::Parse(format!("calamine ods: {}", e)))?;

    let sheets_meta = workbook.worksheets();
    let mut sheets: Vec<XlsxSheet> = Vec::with_capacity(sheets_meta.len().min(MAX_SHEETS));
    for (name, range) in sheets_meta.into_iter().take(MAX_SHEETS) {
        let mut rows_iter = range.rows();
        let header: Vec<String> = rows_iter
            .next()
            .map(|r| r.iter().map(|c| c.to_string()).collect())
            .unwrap_or_default();
        let preview_rows: Vec<Vec<String>> = rows_iter
            .take(200)
            .map(|r| r.iter().map(|c| c.to_string()).collect())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ods_parser_compiles() {
        // Just ensure the module compiles
    }

    #[test]
    fn parses_cell_placement_and_formulas() {
        let xml = r#"
        <office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
            xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"
            xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0">
          <office:body><office:spreadsheet>
            <table:table table:name="Sheet1">
              <table:table-row>
                <table:table-cell office:value-type="string"><text:p>Name</text:p></table:table-cell>
                <table:table-cell office:value-type="string"><text:p>Value</text:p></table:table-cell>
              </table:table-row>
              <table:table-row>
                <table:table-cell office:value-type="string"><text:p>A</text:p></table:table-cell>
                <table:table-cell office:value-type="float" office:value="1.5" table:formula="of:=1+0.5"/>
              </table:table-row>
            </table:table>
            <table:table table:name="MyLinks">
              <table:table-row>
                <table:table-cell table:number-columns-repeated="3"><text:p>x</text:p></table:table-cell>
                <table:table-cell office:value-type="string"><text:p>y</text:p></table:table-cell>
              </table:table-row>
            </table:table>
          </office:spreadsheet></office:body>
        </office:document-content>
        "#;

        let sheets = parse_ods_sheets(xml);
        assert_eq!(sheets.len(), 2);
        assert_eq!(sheets[0].name, "Sheet1");
        assert_eq!(sheets[1].name, "MyLinks");

        let s0 = &sheets[0];
        assert_eq!(s0.total_cols_hint, Some(2));
        assert_eq!(
            s0.preview_formulas.as_ref().unwrap()[1],
            vec!["".to_string(), "=1+0.5".to_string()]
        );
        assert_eq!(s0.preview_rows[1], vec!["A".to_string(), "1.5".to_string()]);

        // A gap (missing cell) keeps placement: 3 repeated → next cell at col 4.
        let s1 = &sheets[1];
        assert_eq!(s1.total_cols_hint, Some(4));
        assert_eq!(
            s1.preview_rows[0],
            vec![
                "x".to_string(),
                "x".to_string(),
                "x".to_string(),
                "y".to_string()
            ]
        );
    }

    #[test]
    fn strips_of_prefix_only() {
        assert_eq!(strip_formula_prefix("of:=SUM(B1:B5)"), "=SUM(B1:B5)");
        assert_eq!(strip_formula_prefix("=SUM(B1:B5)"), "=SUM(B1:B5)");
    }
}
