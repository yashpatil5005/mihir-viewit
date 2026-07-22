//! Phase 2.8 — EPUB. Per plan §4 "near-zero cost: zip+xhtml".
//!
//! Implementation: open the EPUB as a zip, read `META-INF/container.xml`
//! to find the OPF rootfile, parse the OPF spine to get the manifest ids
//! in reading order, name/id map them to entry paths, and ship the FIRST
//! chapter's XHTML eagerly. Subsequent chapters via `epub_chapter` (TODO
//! Phase 5 nav UI).

use std::io::Read;
use viewit_core_types::{Document, Error, Format};
use zip::ZipArchive;

pub fn parse(bytes: &[u8], _format: Format, _name: &str) -> Result<Document, Error> {
    match _format {
        Format::Epub => parse_epub(bytes),
        Format::Mobi => parse_mobi(bytes),
        _ => Err(Error::UnsupportedFormat(_format)),
    }
}

fn parse_epub(bytes: &[u8]) -> Result<Document, Error> {
    let reader = std::io::Cursor::new(bytes);
    let mut archive = ZipArchive::new(reader).map_err(|e| Error::Parse(format!("zip: {}", e)))?;

    // 1) Container → OPF path
    let container = read_entry(&mut archive, "META-INF/container.xml")?;
    let opf_path = parse_container(&container)?;

    // 2) OPF → title, author, spine manifest-ids in reading order
    let opf = read_entry(&mut archive, &opf_path)?;
    let (title, author, spine_paths) = parse_opf(&opf)?;

    // 3) First chapter's XHTML eagerly. The rest is paginated.
    let spine_len = spine_paths.len();
    let first_chapter_xhtml = if let Some(first) = spine_paths.first() {
        read_entry(&mut archive, first).unwrap_or_else(|_| "<p>(empty chapter)</p>".into())
    } else {
        "<p>(no chapters)</p>".into()
    };

    Ok(Document::Epub {
        title,
        author,
        first_chapter_xhtml,
        spine_len,
        byte_len: bytes.len(),
    })
}

fn parse_mobi(bytes: &[u8]) -> Result<Document, Error> {
    let mobi = mobi::Mobi::new(bytes.to_vec())
        .map_err(|e| Error::Parse(format!("mobi parse: {}", e)))?;
    
    let title = mobi.title().to_string();
    let author = mobi.author().map(|s| s.to_string());
    
    // Extract text content — MOBI uses PalmDOC compression
    // The mobi crate handles decompression internally
    let text = mobi.content_as_string()
        .map_err(|e| Error::Parse(format!("mobi text extraction: {}", e)))?;
    let html = if text.is_empty() {
        "<p>(no text content)</p>".into()
    } else {
        // Convert to basic HTML for consistency with EPUB viewer
        let escaped = text.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('\n', "<br>");
        format!("<div>{}</div>", escaped)
    };
    
    Ok(Document::Epub {
        title,
        author,
        first_chapter_xhtml: html,
        spine_len: 1, // MOBI is single-flow
        byte_len: bytes.len(),
    })
}

fn read_entry<R>(archive: &mut ZipArchive<R>, name: &str) -> Result<String, Error>
where
    R: Read + std::io::Seek,
{
    let mut f = archive
        .by_name(name)
        .map_err(|e| Error::Parse(format!("zip entry '{}' not found: {}", name, e)))?;
    let mut s = String::new();
    f.read_to_string(&mut s).map_err(|e| Error::Parse(format!("read: {}", e)))?;
    Ok(s)
}

/// Parse META-INF/container.xml, return the OPF rootfile path (first one).
fn parse_container(xml: &str) -> Result<String, Error> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    let mut opf_path: Option<String> = None;
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(e)) | Ok(Event::Start(e)) => {
                if e.name().as_ref() == b"rootfile" {
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"full-path" {
                            opf_path = Some(String::from_utf8_lossy(attr.value.as_ref()).into_owned());
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(Error::Parse(format!("container.xml parse: {}", e))),
            _ => ()
        }
        buf.clear();
    }
    opf_path.ok_or_else(|| Error::Parse("no <rootfile full-path=> in container.xml".into()))
}

/// Parse OPF (XML), return (title, author, ordered list of entry paths).
fn parse_opf(xml: &str) -> Result<(String, Option<String>, Vec<String>), Error> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    let mut title = String::new();
    let mut author: Option<String> = None;
    let mut manifest: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut spine: Vec<String> = Vec::new();
    let mut in_metadata = false;
    let mut in_title_meta = false;
    let mut in_author_meta = false;
    let mut in_manifest_section = false;
    let mut in_spine_section = false;

    loop {
        let ev = reader.read_event_into(&mut buf);
        match ev {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let name = e.name();
                match name.as_ref() {
                    b"metadata" => in_metadata = true,
                    b"manifest" => in_manifest_section = true,
                    b"spine" => in_spine_section = true,
                    b"item" if in_manifest_section => {
                        let mut id_v = String::new();
                        let mut href_v = String::new();
                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"id" => id_v = String::from_utf8_lossy(attr.value.as_ref()).into_owned(),
                                b"href" => href_v = String::from_utf8_lossy(attr.value.as_ref()).into_owned(),
                                _ => {}
                            }
                        }
                        if !id_v.is_empty() { manifest.insert(id_v, href_v); }
                    }
                    b"itemref" if in_spine_section => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"idref" {
                                spine.push(String::from_utf8_lossy(attr.value.as_ref()).into_owned());
                            }
                        }
                    }
                    b"dc:title" if in_metadata => in_title_meta = true,
                    b"dc:creator" if in_metadata => in_author_meta = true,
                    _ => {}
                }
            }
            Ok(Event::End(e)) => {
                match e.name().as_ref() {
                    b"metadata" => in_metadata = false,
                    b"manifest" => in_manifest_section = false,
                    b"spine" => in_spine_section = false,
                    b"dc:title" => in_title_meta = false,
                    b"dc:creator" => in_author_meta = false,
                    _ => {}
                }
            }
            Ok(Event::Text(t)) => {
                let s = t.unescape().map_err(|e| Error::Parse(format!("esc: {}", e)))?.into_owned();
                if in_title_meta && title.is_empty() { title = s; }
                else if in_author_meta && author.is_none() { author = Some(s); }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(Error::Parse(format!("opf parse: {}", e))),
            _ => {}
        }
        buf.clear();
    }

    // Resolve spine idrefs into the actual file paths via the manifest.
    let spine_paths: Vec<String> = spine
        .iter()
        .filter_map(|id| manifest.get(id).cloned())
        .collect();

    Ok((title, author, spine_paths))
}
