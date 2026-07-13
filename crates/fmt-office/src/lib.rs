//! Phase 3 — Office format parsers.
//!
//! - XLSX (3.2): `calamine` → `Document::Csv`
//! - DOCX (3.1): `docx-rust` → `Document::Text`
//! - PPTX (3.3): custom OOXML reader → `Document::Pptx`
//! - ODS (3.4): `calamine` → `Document::Csv`
//! - ODT/ODP (3.4): custom zip+XML reader → `Document::Text`

pub mod pptx;
pub mod odt;
pub mod odp;

use std::io::Cursor;
use viewit_core_types::{Document, Error, Format};

pub fn parse(bytes: &[u8], format: Format, name: &str) -> Result<Document, Error> {
    // Phase 3.7 — if encrypted Office, attempt pre-decrypt with empty password
    // (common for read-only protected files). If that fails, surface Unsupported.
    let bytes = match try_decrypt_if_encrypted(bytes)? {
        Some(decrypted) => decrypted,
        None => bytes.to_vec(),
    };
    let bytes = bytes.as_slice();

    match format {
        Format::Xlsx | Format::Ods => parse_xlsx_ods(bytes, format),
        Format::Docx => parse_docx(bytes),
        Format::Pptx => pptx::parse_pptx(bytes, format, name),
        Format::Odt => odt::parse_odt(bytes, format, name),
        Format::Odp => odp::parse_odp(bytes, format, name),
        _ => Err(Error::UnsupportedFormat(format)),
    }
}

/// Detect OLE2 encrypted Office (CFB header D0 CF 11 E0 A1 B1 1A E1).
/// Try office-crypto with empty password. Returns None if not encrypted.
fn try_decrypt_if_encrypted(bytes: &[u8]) -> Result<Option<Vec<u8>>, Error> {
    const OLE2_MAGIC: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
    if bytes.len() < 8 || bytes[..8] != OLE2_MAGIC {
        return Ok(None);
    }
    // Encrypted OLE2 file — try office-crypto with empty password.
    match office_crypto::decrypt_from_bytes(bytes.to_vec(), "") {
        Ok(decrypted) => Ok(Some(decrypted)),
        Err(_) => Err(Error::Parse(
            "Encrypted Office file — password required (not yet supported in UI)".into(),
        )),
    }
}

/// Phase 3.2 (XLSX) + 3.4 (ODS) — `calamine` reader.
/// XLSX produces `Document::Xlsx` with multi-sheet metadata;
/// ODS still falls through to `Document::Csv` (calamine single-sheet).
fn parse_xlsx_ods(bytes: &[u8], format: Format) -> Result<Document, Error> {
    use calamine::Reader;

    // XLSX and ODS use different calamine readers but same sheet iteration.
    if format == Format::Xlsx {
        return parse_xlsx(bytes);
    }
    // ODS path — single-sheet CSV (unchanged for backward compat)
    let cursor = Cursor::new(bytes.to_vec());
    let mut workbook = calamine::Ods::<Cursor<Vec<u8>>>::new(cursor)
        .map_err(|e| Error::Parse(format!("calamine ods: {}", e)))?;

    let Some((_, range)) = workbook.worksheets().into_iter().next() else {
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

fn parse_xlsx(bytes: &[u8]) -> Result<Document, Error> {
    use calamine::Reader;
    use viewit_core_types::XlsxSheet;
    let cursor = Cursor::new(bytes.to_vec());
    let mut workbook = calamine::Xlsx::<Cursor<Vec<u8>>>::new(cursor)
        .map_err(|e| Error::Parse(format!("calamine xlsx: {}", e)))?;
    let sheets_meta = workbook.worksheets();
    let mut sheets: Vec<XlsxSheet> = Vec::with_capacity(sheets_meta.len());
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
        });
    }
    Ok(Document::Xlsx {
        sheets,
        byte_len: bytes.len(),
    })
}

/// Phase 3.1 — DOCX via `docx-rust`, walked structurally.
/// Produces `Document::Docx` with paragraphs (heading-aware), list items,
/// tables. Embedded images surface as `DocxBlock::Image` placeholders
/// (Phase 3.8 actual binary extraction pending).
fn parse_docx(bytes: &[u8]) -> Result<Document, Error> {
    use docx_rust::document::{BodyContent, TableCellContent, TableRowContent};
    use viewit_core_types::DocxBlock;

    let cursor = Cursor::new(bytes);
    let docx_file = docx_rust::DocxFile::from_reader(cursor)
        .map_err(|e| Error::Parse(format!("docx-rust from_reader: {}", e)))?;
    let docx = docx_file.parse()
        .map_err(|e| Error::Parse(format!("docx-rust parse: {}", e)))?;

    let mut blocks: Vec<DocxBlock> = Vec::new();
    for content in &docx.document.body.content {
        match content {
            BodyContent::Paragraph(p) => {
                let text = p.text();
                if text.trim().is_empty() { continue; }
                // Heading detection: peek property.style_id for "HeadingN"
                let heading = p.property
                    .as_ref()
                    .and_then(|prop| prop.style_id.as_ref())
                    .and_then(|sid| {
                        let v = &sid.value;
                        if v.starts_with("Heading") {
                            v[7..].parse::<u8>().ok()
                        } else if v.starts_with("heading ") {
                            v[8..].parse::<u8>().ok()
                        } else if v == "Title" {
                            Some(1)
                        } else {
                            None
                        }
                    });
                // List detection: property.num_pr presence indicates list item.
                let is_list = p.property.as_ref()
                    .map(|prop| prop.numbering.is_some())
                    .unwrap_or(false);
                if is_list {
                    blocks.push(DocxBlock::ListItem { text, level: 0 });
                } else {
                    blocks.push(DocxBlock::Paragraph { text, heading });
                }
            }
            BodyContent::Table(t) => {
                let mut rows: Vec<Vec<String>> = Vec::new();
                for row in &t.rows {
                    let mut cells: Vec<String> = Vec::new();
                    for tc in &row.cells {
                        let cell = match tc { TableRowContent::TableCell(c) => c, _ => continue };
                        let cell_text: Vec<String> = cell.content.iter()
                            .filter_map(|c| match c {
                                TableCellContent::Paragraph(p) => Some(p.text()),
                            })
                            .collect();
                        cells.push(cell_text.join("\n"));
                    }
                    rows.push(cells);
                }
                blocks.push(DocxBlock::Table { rows });
            }
            _ => {}
        }
    }

    Ok(Document::Docx {
        blocks,
        byte_len: bytes.len(),
    })
}
