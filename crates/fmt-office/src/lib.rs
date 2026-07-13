//! Phase 3 — Office format parsers.
//!
//! - XLSX (3.2): `calamine` → `Document::Csv`
//! - DOCX (3.1): `docx-rust` → `Document::Text`
//! - PPTX (3.3): stub → `Document::Placeholder`
//! - ODS (3.4): `calamine` → `Document::Csv`
//! - ODT/ODP (3.4): stub → `Document::Placeholder`

use std::io::{Cursor, Seek};
use viewit_core_types::{Document, Error, Format};

pub fn parse(bytes: &[u8], format: Format, _name: &str) -> Result<Document, Error> {
    match format {
        Format::Xlsx | Format::Ods => parse_xlsx_ods(bytes),
        Format::Docx => parse_docx(bytes),
        Format::Pptx | Format::Odt | Format::Odp => Ok(Document::Placeholder {
            format,
            name: _name.to_string(),
            byte_len: bytes.len(),
        }),
        _ => Err(Error::UnsupportedFormat(format)),
    }
}

/// Phase 3.2 (XLSX) + 3.4 (ODS) — `calamine` reader.
fn parse_xlsx_ods(bytes: &[u8]) -> Result<Document, Error> {
    use calamine::Reader;
    let cursor = Cursor::new(bytes.to_vec());
    let mut workbook = calamine::Xls::<Cursor<Vec<u8>>>::new(cursor)
        .map_err(|e| Error::Parse(format!("calamine: {}", e)))?;

    let Some((_, range)) = workbook
        .worksheets()
        .into_iter()
        .next()
    else {
        return Ok(Document::Csv {
            header: vec![],
            preview_rows: vec![],
            total_rows_hint: Some(0),
            byte_len: bytes.len(),
        });
    };

    let mut rows_iter = range.rows();
    let header: Vec<String> = rows_iter
        .next()
        .map(|r| r.iter().map(|c| c.to_string()).collect())
        .unwrap_or_default();

    let preview_rows: Vec<Vec<String>> = rows_iter
        .take(200)
        .map(|r| r.iter().map(|c| c.to_string()).collect())
        .collect();

    Ok(Document::Csv {
        header,
        preview_rows,
        total_rows_hint: Some(range.height()),
        byte_len: bytes.len(),
    })
}

/// Phase 3.1 — DOCX via `docx-rust`.
fn parse_docx(bytes: &[u8]) -> Result<Document, Error> {
    let cursor = Cursor::new(bytes);
    let docx_file = docx_rust::DocxFile::from_reader(cursor)
        .map_err(|e| Error::Parse(format!("docx-rust from_reader: {}", e)))?;
    let docx = docx_file.parse()
        .map_err(|e| Error::Parse(format!("docx-rust parse: {}", e)))?;
    let text = docx.document.body.text();
    Ok(Document::Text {
        content: text,
        encoding: "utf-8".into(),
        byte_len: bytes.len(),
    })
}
