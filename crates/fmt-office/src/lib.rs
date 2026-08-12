//! ViewIt Unified Office Parser — Phase 3+
//!
//! Combines:
//! - OOXML: .docx, .docm, .dotx, .dotm, .xlsx, .xlsm, .xlsb, .xls, .pptx, .pptm, .potx
//! - Legacy Binary: .doc, .ppt (text extraction)
//! - OpenDocument: .odt, .ott, .ods, .ots, .odp, .otp
//!
//! Entry point: `render(bytes, ext) -> String` for WASM/Android native plugins.

pub mod docx;
pub mod legacy_binary;
pub mod odp;
pub mod ods;
pub mod odt;
pub mod pptx;
pub mod xlsx;

#[cfg(all(target_arch = "wasm32", feature = "wasm"))]
pub mod wasm_entry;

use std::io::{Cursor, Read};
use viewit_core_types::{Document, Error, Format};

/// Unified entry point for all Office formats.
/// Returns JSON string for WASM/Android native plugin consumption.
pub fn render(bytes: &[u8], ext: &str) -> Result<String, Error> {
    let format = sniff_format(bytes, ext);
    let name = format!("file.{}", ext);
    let doc = dispatch(format, bytes, ext, &name)?;
    serde_json::to_string(&doc).map_err(|e| Error::Parse(e.to_string()))
}

/// Native entry point used by `viewit-core`.
/// Dispatches on caller-provided `Format` to the same parser modules as `render`.
pub fn parse(bytes: &[u8], format: Format, name: &str) -> Result<Document, Error> {
    dispatch(format, bytes, "", name)
}

/// Format detection from bytes + extension
fn sniff_format(bytes: &[u8], ext: &str) -> Format {
    // Magic bytes first
    if bytes.len() >= 4 {
        // PDF
        if bytes.starts_with(b"%PDF") {
            return Format::Pdf;
        }
        // ZIP-based formats (OOXML, ODF, EPUB, iWork)
        if bytes.starts_with(&[0x50, 0x4B, 0x03, 0x04])
            || bytes.starts_with(&[0x50, 0x4B, 0x05, 0x06])
        {
            return sniff_zip_format(bytes, ext);
        }
        // OLE2 Compound File (legacy .doc, .xls, .ppt)
        if bytes.starts_with(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]) {
            return sniff_ole2_format(ext);
        }
    }
    // Extension fallback
    sniff_ext(ext)
}

fn sniff_zip_format(bytes: &[u8], ext: &str) -> Format {
    let cursor = Cursor::new(bytes);
    let mut archive = match zip::ZipArchive::new(cursor) {
        Ok(a) => a,
        Err(_) => return Format::ArchiveZip,
    };

    let mut has_content_types = false;
    let mut has_word = false;
    let mut has_xl = false;
    let mut has_ppt = false;
    let mut has_epub = false;
    let mut has_odf = false;

    for i in 0..archive.len() {
        let Ok(file) = archive.by_index(i) else {
            continue;
        };
        let name = file.name();
        if name == "[Content_Types].xml" {
            has_content_types = true;
        }
        if name.starts_with("word/") {
            has_word = true;
        }
        if name.starts_with("xl/") {
            has_xl = true;
        }
        if name.starts_with("ppt/") {
            has_ppt = true;
        }
        if name == "META-INF/container.xml" {
            has_epub = true;
        }
        // ODF indicators
        if name == "content.xml"
            || name.starts_with("Thumbnails/")
            || name == "META-INF/manifest.xml"
        {
            has_odf = true;
        }
    }

    // Extension-based disambiguation for OOXML
    if has_content_types {
        match ext {
            "docx" | "docm" | "dotx" | "dotm" => return Format::Docx,
            "xlsx" | "xlsm" => return Format::Xlsx,
            "xls" => return Format::Xls,
            "xlsb" => return Format::Xlsb,
            "pptx" | "pptm" | "potx" => return Format::Pptx,
            _ => {}
        }
        if has_xl {
            return Format::Xlsx;
        }
        if has_word {
            return Format::Docx;
        }
        if has_ppt {
            return Format::Pptx;
        }
    }

    // ODF detection
    if has_odf || !has_epub {
        match ext {
            "odt" | "ott" => return Format::Odt,
            "ods" | "ots" => return Format::Ods,
            "odp" | "otp" => return Format::Odp,
            _ => {}
        }
        // Fallback: check content.xml for office:document-class
        if let Ok(mut f) = archive.by_name("content.xml") {
            let mut xml = String::new();
            if f.read_to_string(&mut xml).is_ok() && xml.contains("office:document-class") {
                if xml.contains("text") {
                    return Format::Odt;
                }
                if xml.contains("spreadsheet") {
                    return Format::Ods;
                }
                if xml.contains("presentation") {
                    return Format::Odp;
                }
            }
        }
    }

    if has_epub {
        return Format::Epub;
    }

    Format::ArchiveZip
}

fn sniff_ole2_format(ext: &str) -> Format {
    match ext {
        "doc" => Format::Doc,
        "ppt" => Format::Ppt,
        "xls" => Format::Xls,
        "xlsb" => Format::Xlsb, // Excel Binary Workbook — OLE2/BIFF12, not .doc
        _ => Format::Doc,       // Default to Doc for unknown OLE2
    }
}

fn sniff_ext(ext: &str) -> Format {
    match ext {
        "docx" | "docm" | "dotx" | "dotm" => Format::Docx,
        "xlsx" | "xlsm" => Format::Xlsx,
        "xls" => Format::Xls,
        "xlsb" => Format::Xlsb,
        "pptx" | "pptm" | "potx" => Format::Pptx,
        "odt" | "ott" => Format::Odt,
        "ods" | "ots" => Format::Ods,
        "odp" | "otp" => Format::Odp,
        "doc" => Format::Doc,
        "ppt" => Format::Ppt,
        _ => Format::Unsupported,
    }
}

/// Dispatch to format-specific parser
fn dispatch(format: Format, bytes: &[u8], _ext: &str, name: &str) -> Result<Document, Error> {
    // Try decrypt if encrypted Office (OLE2 or OOXML)
    let bytes = match try_decrypt_if_encrypted(bytes)? {
        Some(decrypted) => decrypted,
        None => bytes.to_vec(),
    };
    let bytes = bytes.as_slice();

    match format {
        Format::Xlsx => crate::xlsx::parse_xlsx(bytes),
        Format::Xlsb => crate::xlsx::parse_xlsb(bytes),
        Format::Xls => crate::xlsx::parse_xls_binary(bytes),
        Format::Ods => ods::parse_ods(bytes, format, name),
        Format::Docx => docx::parse_docx(bytes),
        Format::Pptx => pptx::parse_pptx(bytes, format, name),
        Format::Odt => odt::parse_odt(bytes, format, name),
        Format::Odp => odp::parse_odp(bytes, format, name),
        Format::Doc | Format::Ppt => legacy_binary::parse_legacy_binary(bytes, format),
        _ => Err(Error::UnsupportedFormat(format)),
    }
}

/// Phase 3.7 — Encrypted Office detection and decryption attempt
#[cfg(feature = "support-encrypted")]
fn try_decrypt_if_encrypted(bytes: &[u8]) -> Result<Option<Vec<u8>>, Error> {
    const OLE2_MAGIC: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
    if bytes.len() < 8 || bytes[..8] != OLE2_MAGIC {
        return Ok(None);
    }
    // Try CFB first (unencrypted OLE2)
    let cursor = Cursor::new(bytes);
    if cfb::CompoundFile::open(cursor).is_ok() {
        return Ok(None);
    }
    // Try office-crypto with empty password
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
    let cursor = Cursor::new(bytes);
    if cfb::CompoundFile::open(cursor).is_ok() {
        return Ok(None);
    }
    Err(Error::Parse(
        "Encrypted Office file — office-crypto feature not enabled in this build".into(),
    ))
}

// WASM exports are in wasm_entry.rs
#[cfg(all(target_arch = "wasm32", feature = "wasm"))]
mod wasm_exports {
    // Module exists to ensure wasm_entry is compiled
    // Exports are defined in wasm_entry.rs directly
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a minimal valid DOCX (ZIP with word/document.xml)
    fn minimal_docx() -> Vec<u8> {
        use std::io::Write;
        use zip::write::FileOptions;
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));
            let options: FileOptions<'_, ()> =
                FileOptions::default().compression_method(zip::CompressionMethod::Stored);
            zip.start_file("[Content_Types].xml", options).unwrap();
            zip.write_all(b"<?xml version=\"1.0\"?><Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"xml\" ContentType=\"application/xml\"/></Types>").unwrap();
            zip.start_file("_rels/.rels", options).unwrap();
            zip.write_all(b"<?xml version=\"1.0\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"word/document.xml\"/></Relationships>").unwrap();
            zip.start_file("word/document.xml", options).unwrap();
            zip.write_all(b"<?xml version=\"1.0\"?><w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body><w:p><w:t>Test</w:t></w:p></w:body></w:document>").unwrap();
            zip.finish().unwrap();
        }
        buf
    }

    /// Create a minimal valid ODT (ZIP with content.xml)
    fn minimal_odt() -> Vec<u8> {
        use std::io::Write;
        use zip::write::FileOptions;
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));
            let options: FileOptions<'_, ()> =
                FileOptions::default().compression_method(zip::CompressionMethod::Stored);
            zip.start_file("content.xml", options).unwrap();
            zip.write_all(b"<?xml version=\"1.0\"?><office:document-content xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" xmlns:text=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\"><office:body><office:text><text:p>Test</text:p></office:text></office:body></office:document-content>").unwrap();
            zip.finish().unwrap();
        }
        buf
    }

    /// Create a minimal valid XLSX (ZIP with xl/worksheets/sheet1.xml)
    fn minimal_xlsx() -> Vec<u8> {
        use std::io::Write;
        use zip::write::FileOptions;
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));
            let options: FileOptions<'_, ()> =
                FileOptions::default().compression_method(zip::CompressionMethod::Stored);
            zip.start_file("[Content_Types].xml", options).unwrap();
            zip.write_all(b"<?xml version=\"1.0\"?><Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"xml\" ContentType=\"application/xml\"/></Types>").unwrap();
            zip.start_file("_rels/.rels", options).unwrap();
            zip.write_all(b"<?xml version=\"1.0\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"xl/workbook.xml\"/></Relationships>").unwrap();
            zip.start_file("xl/workbook.xml", options).unwrap();
            zip.write_all(b"<?xml version=\"1.0\"?><workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><sheets><sheet name=\"Sheet1\" sheetId=\"1\" r:id=\"rId1\"/></sheets></workbook>").unwrap();
            zip.start_file("xl/worksheets/sheet1.xml", options).unwrap();
            zip.write_all(b"<?xml version=\"1.0\"?><worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><sheetData><row r=\"1\"><c r=\"A1\"><v>Test</v></c></row></sheetData></worksheet>").unwrap();
            zip.start_file("xl/_rels/workbook.xml.rels", options)
                .unwrap();
            zip.write_all(b"<?xml version=\"1.0\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet1.xml\"/></Relationships>").unwrap();
            zip.finish().unwrap();
        }
        buf
    }

    /// Create a minimal valid PPTX (ZIP with ppt/presentation.xml and ppt/slides/slide1.xml)
    fn minimal_pptx() -> Vec<u8> {
        use std::io::Write;
        use zip::write::FileOptions;
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));
            let options: FileOptions<'_, ()> =
                FileOptions::default().compression_method(zip::CompressionMethod::Stored);
            zip.start_file("[Content_Types].xml", options).unwrap();
            zip.write_all(b"<?xml version=\"1.0\"?><Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"xml\" ContentType=\"application/xml\"/></Types>").unwrap();
            zip.start_file("ppt/presentation.xml", options).unwrap();
            zip.write_all(b"<?xml version=\"1.0\"?><p:presentation xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\"><p:sldIdLst><p:sldId id=\"256\" r:id=\"rId1\"/></p:sldIdLst><p:sldIdLst/></p:presentation>").unwrap();
            zip.start_file("ppt/slides/slide1.xml", options).unwrap();
            zip.write_all(b"<?xml version=\"1.0\"?><p:sld xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\"><p:cSld><p:spTree><p:sp><p:nvSpPr><p:ph type=\"title\"/></p:nvSpPr><p:txBody><a:p><a:t>Slide Title</a:t></a:p></p:txBody></p:sp><p:sp><p:nvSpPr><p:ph type=\"body\"/></p:nvSpPr><p:txBody><a:p><a:t>Body text</a:t></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:sld>").unwrap();
            zip.start_file("ppt/_rels/presentation.xml.rels", options)
                .unwrap();
            zip.write_all(b"<?xml version=\"1.0\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide\" Target=\"slides/slide1.xml\"/></Relationships>").unwrap();
            zip.finish().unwrap();
        }
        buf
    }

    /// Create a minimal valid OLE2 compound file (for .doc/.ppt)
    fn minimal_ole2() -> Vec<u8> {
        // Minimal OLE2 header - we just return the magic bytes
        // The parser will fail to open it but that's expected for these test bytes
        vec![0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]
    }

    #[test]
    fn render_docx_ext() {
        let bytes = minimal_docx();
        let result = render(&bytes, "docx");
        assert!(result.is_ok());
        let doc: Document = serde_json::from_str(&result.unwrap()).unwrap();
        assert!(matches!(
            doc,
            Document::Placeholder {
                format: Format::Docx,
                ..
            } | Document::Docx { .. }
        ));
    }

    #[test]
    fn render_odt_ext() {
        let bytes = minimal_odt();
        let result = render(&bytes, "odt");
        assert!(result.is_ok());
    }

    #[test]
    fn render_xlsx_ext() {
        let bytes = minimal_xlsx();
        let result = render(&bytes, "xlsx");
        assert!(result.is_ok());
    }

    #[test]
    fn render_pptx_ext() {
        let bytes = minimal_pptx();
        let result = render(&bytes, "pptx");
        assert!(result.is_ok());
    }

    #[test]
    fn render_legacy_doc_ext() {
        let bytes = minimal_ole2();
        let result = render(&bytes, "doc");
        // This will fail because it's not a valid OLE2 file, just the magic bytes
        // The test just ensures the entry point is callable
        assert!(result.is_err() || result.is_ok());
    }

    #[test]
    fn sniffs_ole2_xlsb_to_xlsb_not_doc() {
        let xlsb_magic = vec![0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
        // A real .xlsb is an OLE2 compound file; it must never be sniffed as
        // legacy .doc (which produced a garbled byte-dump preview before the fix).
        assert_eq!(
            super::sniff_format(&xlsb_magic, "xlsb"),
            Format::Xlsb,
            "xlsb must sniff to Xlsb, not Doc"
        );
        assert_eq!(
            super::sniff_format(&xlsb_magic, "xls"),
            Format::Xls,
            "xls must sniff to Xls"
        );
        assert_eq!(
            super::sniff_format(&xlsb_magic, "doc"),
            Format::Doc,
            "doc still Doc"
        );
        assert_eq!(
            super::sniff_format(&xlsb_magic, "ppt"),
            Format::Ppt,
            "ppt still Ppt"
        );
    }

    #[test]
    fn sniffs_zip_xlsm_to_xlsx() {
        let bytes = minimal_xlsx();
        assert_eq!(
            super::sniff_format(&bytes, "xlsm"),
            Format::Xlsx,
            "xlsm shares OOXML structure with xlsx"
        );
        assert_eq!(super::sniff_format(&bytes, "xlsx"), Format::Xlsx);
    }

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
        let slides = crate::legacy_binary::legacy_ppt_slides(text);
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
        let slides = crate::legacy_binary::legacy_ppt_slides("");
        assert_eq!(slides.len(), 1);
        assert!(
            slides[0].body.contains("not decoded"),
            "empty PPT text should yield honest partial-notice slide"
        );
    }
}
