//! XLSX / XLSB / XLS parser — unified for office-universal.
//!
//! Uses calamine for all three formats.
//! Produces Document::Xlsx with multi-sheet metadata.

use std::io::Cursor;
use calamine::Reader;
use viewit_core_types::{Document, Error, XlsxSheet};

pub fn parse_xlsx(bytes: &[u8]) -> Result<Document, Error> {
    let cursor = Cursor::new(bytes.to_vec());
    let mut workbook = calamine::Xlsx::<Cursor<Vec<u8>>>::new(cursor)
        .map_err(|e| Error::Parse(format!("calamine xlsx: {}", e)))?;
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
            preview_formulas: None,
        });
    }
    Ok(Document::Xlsx {
        sheets,
        byte_len: bytes.len(),
    })
}

pub fn parse_xlsm(bytes: &[u8]) -> Result<Document, Error> {
    // XLSB uses a different reader in calamine, but for now fallback to xlsx
    // since they share the same OOXML structure for worksheets
    parse_xlsx(bytes)
}

pub fn parse_xlsb(bytes: &[u8]) -> Result<Document, Error> {
    // XLSB (binary) - calamine supports Xlsb reader
    let cursor = Cursor::new(bytes.to_vec());
    let mut workbook = calamine::Xlsb::<Cursor<Vec<u8>>>::new(cursor)
        .map_err(|e| Error::Parse(format!("calamine xlsb: {}", e)))?;
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
            preview_formulas: None,
        });
    }
    Ok(Document::Xlsx {
        sheets,
        byte_len: bytes.len(),
    })
}

pub fn parse_xls_binary(bytes: &[u8]) -> Result<Document, Error> {
    let cursor = Cursor::new(bytes.to_vec());
    let mut workbook = calamine::Xls::<Cursor<Vec<u8>>>::new(cursor)
        .map_err(|e| Error::Parse(format!("calamine xls: {}", e)))?;
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
            preview_formulas: None,
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
    fn xlsx_parser_compiles() {
        // Just ensure the module compiles
    }
}