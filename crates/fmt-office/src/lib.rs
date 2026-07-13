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
        Format::Xlsx | Format::Ods => parse_xlsx_ods(bytes),
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
