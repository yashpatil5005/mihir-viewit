//! ODS / OTS parser (OpenDocument Spreadsheet).
//!
//! Uses calamine's Ods reader.
//! Produces Document::Xlsx with multi-sheet metadata (same as XLSX).

use std::io::Cursor;
use calamine::Reader;
use viewit_core_types::{Document, Error, Format, XlsxSheet};

pub fn parse_ods(bytes: &[u8], _format: Format, _name: &str) -> Result<Document, Error> {
    let cursor = Cursor::new(bytes.to_vec());
    let mut workbook = calamine::Ods::<Cursor<Vec<u8>>>::new(cursor)
        .map_err(|e| Error::Parse(format!("calamine ods: {}", e)))?;

    let sheets_meta = workbook.worksheets();
    let mut sheets: Vec<XlsxSheet> = Vec::with_capacity(sheets_meta.len().min(8));
    for (name, range) in sheets_meta.into_iter().take(8) {
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
        });
    }

    Ok(Document::Xlsx {
        sheets,
        byte_len: bytes.len(),
    })
}

#[cfg(test)]
mod tests {

    #[test]
    fn ods_parser_compiles() {
        // Just ensure the module compiles
    }
}