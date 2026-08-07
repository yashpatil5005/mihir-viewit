//! Phase 3 — Office format parsers.
//!
//! - XLSX (3.2): `calamine` → `Document::Csv`
//! - DOCX (3.1): `docx-rust` + WordprocessingML fallback → `Document::Docx`
//! - PPTX (3.3): custom OOXML reader → `Document::Pptx`
//! - ODS (3.4): `calamine` → `Document::Csv`
//! - ODT/ODP (3.4): custom zip+XML reader → `Document::Text`

pub mod odp;
pub mod odt;
pub mod pptx;

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
        Format::Xls => parse_xls_binary(bytes),
        Format::Docx => parse_docx(bytes),
        Format::Pptx => pptx::parse_pptx(bytes, format, name),
        Format::Odt => odt::parse_odt(bytes, format, name),
        Format::Odp => odp::parse_odp(bytes, format, name),
        // Phase 3.6 — legacy binary Office (.doc / .ppt). Best-effort text
        // extraction via CFB compound-storage walk. Per plan §4: "best-effort".
        Format::Doc | Format::Ppt => parse_legacy_binary(bytes, format),
        _ => Err(Error::UnsupportedFormat(format)),
    }
}

/// Detect OLE2 encrypted Office (CFB header D0 CF 11 E0 A1 B1 1A E1).
/// Try office-crypto with empty password. Returns None if not encrypted.
#[cfg(feature = "support-encrypted")]
fn try_decrypt_if_encrypted(bytes: &[u8]) -> Result<Option<Vec<u8>>, Error> {
    const OLE2_MAGIC: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
    if bytes.len() < 8 || bytes[..8] != OLE2_MAGIC {
        return Ok(None);
    }
    // All OLE2 files share the same header — encrypted or not.
    // First try to open with cfb directly; if it succeeds, the file is not encrypted.
    let cursor = std::io::Cursor::new(bytes);
    if cfb::CompoundFile::open(cursor).is_ok() {
        return Ok(None);
    }
    // cfb failed — might be encrypted. Try office-crypto with empty password.
    match office_crypto::decrypt_from_bytes(bytes.to_vec(), "") {
        Ok(decrypted) => Ok(Some(decrypted)),
        Err(_) => Err(Error::Parse(
            "Encrypted Office file — password required (not yet supported in UI)".into(),
        )),
    }
}

#[cfg(not(feature = "support-encrypted"))]
fn try_decrypt_if_encrypted(bytes: &[u8]) -> Result<Option<Vec<u8>>, Error> {
    const OLE2_MAGIC: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
    if bytes.len() < 8 || bytes[..8] != OLE2_MAGIC {
        return Ok(None);
    }
    // Without office-crypto, try opening with cfb directly
    let cursor = std::io::Cursor::new(bytes);
    if cfb::CompoundFile::open(cursor).is_ok() {
        return Ok(None);
    }
    Err(Error::Parse(
        "Encrypted Office file — office-crypto feature not enabled in this build".into(),
    ))
}

/// Phase 3.6 — legacy binary `.xls` via calamine's Xls reader.
fn parse_xls_binary(bytes: &[u8]) -> Result<Document, Error> {
    use calamine::Reader;
    use viewit_core_types::XlsxSheet;
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

/// Phase 3.6 — legacy binary .doc / .ppt best-effort.
/// Uses `cfb` to walk compound storage, then extracts readable UTF-16 LE
/// (typical for Word's WordDocument / PowerPoint Document streams) and ASCII
/// sequences from the other streams. Marked partial-preview.
fn parse_legacy_binary(bytes: &[u8], format: Format) -> Result<Document, Error> {
    let cursor = std::io::Cursor::new(bytes.to_vec());
    let mut cfb =
        cfb::CompoundFile::open(cursor).map_err(|e| Error::Parse(format!("cfb open: {}", e)))?;

    // Stream targets per format — try primary then fallback streams.
    let streams: Vec<&str> = match format {
        Format::Doc => vec!["WordDocument", "1Table", "0Table"],
        Format::Ppt => vec!["PowerPoint Document", "Current User"],
        _ => return Err(Error::UnsupportedFormat(format)),
    };

    let mut text = String::new();
    for stream_name in &streams {
        if cfb.is_stream(stream_name) {
            use std::io::Read;
            if let Ok(mut stream) = cfb.open_stream(stream_name) {
                let mut buf = Vec::new();
                if stream.read_to_end(&mut buf).is_ok() {
                    let extracted = extract_utf16_le(&buf);
                    if !extracted.trim().is_empty() {
                        text.push_str(&extracted);
                        text.push('\n');
                    }
                    let ascii = extract_ascii(&buf);
                    if !ascii.trim().is_empty() {
                        text.push_str(&ascii);
                        text.push('\n');
                    }
                }
            }
        }
    }
    // Fallback: walk all streams and harvest ASCII prints if primary stream empty.
    if text.trim().is_empty() {
        let paths: Vec<_> = cfb
            .read_root_storage()
            .filter(|e| e.is_stream())
            .map(|e| e.path().to_path_buf())
            .collect();
        for path in paths {
            if let Ok(mut s) = cfb.open_stream(&path) {
                use std::io::Read;
                let mut b = Vec::new();
                if s.read_to_end(&mut b).is_ok() {
                    text.push_str(&extract_ascii(&b));
                }
            }
        }
    }

    if format == Format::Ppt {
        let slides = legacy_ppt_slides(&text);
        return Ok(Document::Pptx {
            slide_count: slides.len(),
            slides,
            byte_len: bytes.len(),
            asset_path: String::new(),
            stream_url: None,
        });
    }

    let label = match format {
        Format::Doc => "Word .doc",
        _ => "legacy",
    };
    Ok(Document::Text {
        content: format!(
            "[Partial preview — {} legacy binary, layout/formatting not preserved]\n\n{}",
            label,
            text.trim()
        ),
        encoding: "utf-8".into(),
        byte_len: bytes.len(),
        truncated: text.len() > 256 * 1024,
        stream_url: None,
    })
}

fn legacy_ppt_slides(text: &str) -> Vec<viewit_core_types::PptxSlide> {
    use viewit_core_types::PptxSlide;

    let cleaned: Vec<String> = text
        .lines()
        .map(str::trim)
        .filter(|line| line.len() >= 3)
        .filter(|line| {
            !line
                .chars()
                .all(|c| c.is_ascii_punctuation() || c.is_ascii_digit())
        })
        .map(str::to_string)
        .collect();

    if cleaned.is_empty() {
        return vec![PptxSlide {
            title: "Legacy PowerPoint preview".into(),
            body: "No extractable slide text found. Legacy binary PowerPoint layout is not decoded in the lightweight viewer.".into(),
            elements: Vec::new(),
        }];
    }

    let mut slides = Vec::new();
    let chunk_size = 12;
    for (idx, chunk) in cleaned.chunks(chunk_size).enumerate() {
        let title = chunk
            .first()
            .cloned()
            .filter(|s| s.len() <= 120)
            .unwrap_or_else(|| format!("Legacy PowerPoint slide {}", idx + 1));
        let body = chunk.iter().skip(1).cloned().collect::<Vec<_>>().join("\n");
        slides.push(PptxSlide {
            title,
            body,
            elements: Vec::new(),
        });
    }
    slides
}

/// Extract printable UTF-16 LE strings from a byte slice.
fn extract_utf16_le(bytes: &[u8]) -> String {
    let mut out = String::new();
    let mut cur = String::new();
    let mut i = 0;
    while i + 1 < bytes.len() {
        let lo = bytes[i] as u16;
        let hi = bytes[i + 1] as u16;
        let cp = lo | (hi << 8);
        if (0x20..=0x7E).contains(&cp) {
            cur.push(cp as u8 as char);
        } else if cp == 0x0A || cp == 0x0D {
            if !cur.is_empty() {
                cur.push('\n');
                out.push_str(&cur);
                cur.clear();
            }
        } else if !cur.is_empty() {
            if cur.len() >= 4 {
                out.push_str(&cur);
                out.push('\n');
            }
            cur.clear();
        }
        i += 2;
    }
    if cur.len() >= 4 {
        out.push_str(&cur);
    }
    out
}

/// Extract printable ASCII strings (len >= 4) from a byte slice.
fn extract_ascii(bytes: &[u8]) -> String {
    let mut out = String::new();
    let mut cur: Vec<u8> = Vec::new();
    for &b in bytes {
        if (0x20..=0x7E).contains(&b) {
            cur.push(b);
        } else if b == 0x0A || b == 0x0D {
            if cur.len() >= 4 {
                out.push_str(&String::from_utf8_lossy(&cur));
                out.push('\n');
            }
            cur.clear();
        } else if !cur.is_empty() {
            if cur.len() >= 4 {
                out.push_str(&String::from_utf8_lossy(&cur));
                out.push(' ');
            }
            cur.clear();
        }
    }
    if cur.len() >= 4 {
        out.push_str(&String::from_utf8_lossy(&cur));
    }
    out
}

/// Phase 3.2 (XLSX) + 3.4 (ODS) — `calamine` reader.
/// Both produce `Document::Xlsx` with multi-sheet metadata.
fn parse_xlsx_ods(bytes: &[u8], format: Format) -> Result<Document, Error> {
    use calamine::Reader;
    use viewit_core_types::XlsxSheet;

    if format == Format::Xlsx {
        return parse_xlsx(bytes);
    }

    // ODS path — multi-sheet via calamine Ods reader
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
            preview_formulas: None,
        });
    }

    Ok(Document::Xlsx {
        sheets,
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
            total_cols_hint: Some(range.width()),
            preview_formulas: None,
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
    let docx = match docx_file.parse() {
        Ok(docx) => docx,
        Err(e) => {
            return parse_docx_ooxml_fallback(bytes).map_err(|fallback| {
                Error::Parse(format!("docx-rust parse: {}; fallback: {}", e, fallback))
            })
        }
    };

    let mut blocks: Vec<DocxBlock> = Vec::new();
    for content in &docx.document.body.content {
        match content {
            BodyContent::Paragraph(p) => {
                let text = p.text();
                if text.trim().is_empty() {
                    continue;
                }
                let heading = p
                    .property
                    .as_ref()
                    .and_then(|prop| prop.style_id.as_ref())
                    .and_then(|sid| heading_from_style(&sid.value));
                let is_list = p
                    .property
                    .as_ref()
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
                        let cell = match tc {
                            TableRowContent::TableCell(c) => c,
                            _ => continue,
                        };
                        let cell_text: Vec<String> = cell
                            .content
                            .iter()
                            .map(|c| match c {
                                TableCellContent::Paragraph(p) => p.text(),
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

fn parse_docx_ooxml_fallback(bytes: &[u8]) -> Result<Document, Error> {
    use std::io::Read;

    let cursor = Cursor::new(bytes);
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|e| Error::Parse(format!("docx zip: {}", e)))?;
    let mut xml = String::new();
    archive
        .by_name("word/document.xml")
        .map_err(|e| Error::Parse(format!("docx missing document.xml: {}", e)))?
        .read_to_string(&mut xml)
        .map_err(|e| Error::Parse(format!("docx document.xml read: {}", e)))?;

    parse_docx_document_xml(&xml, bytes.len())
}

fn parse_docx_document_xml(xml: &str, byte_len: usize) -> Result<Document, Error> {
    use quick_xml::events::Event;
    use quick_xml::Reader;
    use viewit_core_types::DocxBlock;

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut blocks = Vec::new();
    let mut in_paragraph = false;
    let mut in_table = false;
    let mut in_cell = false;
    let mut in_text = false;
    let mut paragraph_text = String::new();
    let mut paragraph_heading: Option<u8> = None;
    let mut paragraph_is_list = false;
    let mut cell_text = String::new();
    let mut current_row: Vec<String> = Vec::new();
    let mut table_rows: Vec<Vec<String>> = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = e.name().as_ref().to_vec();
                match name.as_slice() {
                    b"w:tbl" => {
                        in_table = true;
                        table_rows.clear();
                    }
                    b"w:tr" if in_table => {
                        current_row.clear();
                    }
                    b"w:tc" if in_table => {
                        in_cell = true;
                        cell_text.clear();
                    }
                    b"w:p" => {
                        in_paragraph = true;
                        paragraph_text.clear();
                        paragraph_heading = None;
                        paragraph_is_list = false;
                    }
                    b"w:t" => in_text = true,
                    b"w:pStyle" if in_paragraph => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"w:val" {
                                let value =
                                    String::from_utf8_lossy(attr.value.as_ref()).to_string();
                                paragraph_heading = heading_from_style(&value);
                            }
                        }
                    }
                    b"w:numPr" if in_paragraph => paragraph_is_list = true,
                    _ => {}
                }
            }
            Ok(Event::Text(e)) if in_text => {
                let text = e.unescape().unwrap_or_default().to_string();
                if in_cell {
                    cell_text.push_str(&text);
                } else if in_paragraph {
                    paragraph_text.push_str(&text);
                }
            }
            Ok(Event::End(e)) => {
                let name = e.name().as_ref().to_vec();
                match name.as_slice() {
                    b"w:t" => in_text = false,
                    b"w:p" => {
                        if !in_table {
                            let text = paragraph_text.trim().to_string();
                            if !text.is_empty() {
                                if paragraph_is_list {
                                    blocks.push(DocxBlock::ListItem { text, level: 0 });
                                } else {
                                    blocks.push(DocxBlock::Paragraph {
                                        text,
                                        heading: paragraph_heading,
                                    });
                                }
                            }
                        }
                        in_paragraph = false;
                    }
                    b"w:tc" if in_table => {
                        current_row.push(cell_text.trim().to_string());
                        in_cell = false;
                    }
                    b"w:tr" if in_table => {
                        table_rows.push(current_row.clone());
                    }
                    b"w:tbl" => {
                        if !table_rows.is_empty() {
                            blocks.push(DocxBlock::Table {
                                rows: table_rows.clone(),
                            });
                        }
                        in_table = false;
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(Error::Parse(format!("docx xml: {}", e))),
            _ => {}
        }
        buf.clear();
    }

    Ok(Document::Docx { blocks, byte_len })
}

fn heading_from_style(style: &str) -> Option<u8> {
    if style == "Title" {
        return Some(1);
    }
    style
        .strip_prefix("Heading")
        .or_else(|| style.strip_prefix("heading"))
        .and_then(|level| level.trim().parse::<u8>().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn legacy_ppt_text() -> Vec<u8> {
        use std::io::{Cursor, Write};
        let mut buf: Vec<u8> = Vec::new();
        {
            let cursor = Cursor::new(&mut buf);
            let mut cfb = cfb::CompoundFile::create(cursor).unwrap();
            let mut stream = cfb.create_stream("PowerPoint Document").unwrap();
            for line in [
                "Click to edit the title text format",
                "Click to edit the outline text format",
                "Second Outline Level",
                "Third Outline Level",
                "Fourth Outline Level",
                "Arial",
            ] {
                for ch in line.chars() {
                    let _ = stream.write_all(&[ch as u8, 0]);
                }
                let _ = stream.write_all(&[0x0A, 0x00]);
            }
            stream.flush().unwrap();
            drop(stream);
            cfb.flush().unwrap();
        }
        buf
    }

    #[test]
    fn legacy_ppt_maps_to_pptx_slide_model() {
        let bytes = legacy_ppt_text();
        if bytes.is_empty() {
            eprintln!("skipping legacy_ppt_maps_to_pptx_slide_model: cfb stream write unavailable in environment");
            return;
        }
        let doc = crate::parse(&bytes, Format::Ppt, "sample.ppt").unwrap();
        match doc {
            Document::Pptx { slides, .. } => {
                assert!(!slides.is_empty(), "PPT should produce at least one slide");
                assert!(
                    slides.iter().any(|s| !s.title.is_empty()),
                    "slides should have non-empty titles derived from extracted text"
                );
                assert!(
                    slides.iter().any(|s| !s.body.is_empty()),
                    "slides should have non-empty bodies derived from extracted text"
                );
            }
            other => panic!("expected Document::Pptx for PPT, got {:?}", other),
        }
    }

    #[test]
    fn legacy_ppt_slides_chunks_extracted_text() {
        let text = "Alpha\nBeta\nGamma\nDelta\nEpsilon\nZeta\nEta\nTheta\nIota\nKappa\nLambda\nMu\nNu\nXi\nOmicron\nPi\nRho\n";
        let slides = legacy_ppt_slides(text);
        assert!(
            slides.len() >= 2,
            "expected multiple slides from chunking, got {}",
            slides.len()
        );
        assert!(
            slides.iter().all(|s| !s.title.is_empty()),
            "every slide should have a non-empty title"
        );
        assert!(
            slides.iter().any(|s| !s.body.is_empty()),
            "at least one slide should have a non-empty body"
        );
    }

    #[test]
    fn legacy_ppt_slides_empty_text_yields_honest_partial_slide() {
        let slides = legacy_ppt_slides("");
        assert_eq!(slides.len(), 1);
        assert!(
            slides[0].body.contains("not decoded"),
            "empty PPT text should yield honest partial-notice slide"
        );
    }
}
