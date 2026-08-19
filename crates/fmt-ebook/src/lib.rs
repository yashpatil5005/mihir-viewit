//! Phase 2.8 — EPUB. Per plan §4 "near-zero cost: zip+xhtml".
//!
//! Implementation: open the EPUB as a zip, read `META-INF/container.xml`
//! to find the OPF rootfile, parse the OPF spine to get the manifest ids
//! in reading order, name/id map them to entry paths, and ship the FIRST
//! chapter's XHTML eagerly. Subsequent chapters via `epub_chapter` (TODO
//! Phase 5 nav UI).

use std::io::Read;

use base64::{engine::general_purpose, Engine as _};
use viewit_core_types::{Document, Error, Format};
use zip::ZipArchive;

#[cfg(all(target_arch = "wasm32", feature = "wasm"))]
pub mod wasm_entry;

/// WASM/native-plugin entry point: map the extension to a Format, parse, JSON.
pub fn render(bytes: &[u8], ext: &str) -> Result<String, Error> {
    let format = match ext {
        "epub" => Format::Epub,
        "mobi" => Format::Mobi,
        "azw3" => Format::Azw3,
        _ => return Err(Error::UnsupportedFormat(Format::Epub)),
    };
    let doc = parse(bytes, format, "ebook")?;
    serde_json::to_string(&doc).map_err(|e| Error::Parse(e.to_string()))
}

pub fn parse(bytes: &[u8], _format: Format, _name: &str) -> Result<Document, Error> {
    match _format {
        Format::Epub => parse_epub(bytes),
        Format::Mobi | Format::Azw3 => parse_mobi(bytes),
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
    let (title, author, spine_paths) = parse_opf(&opf, &opf_path)?;

    // 3) First chapter's XHTML eagerly. The rest is paginated.
    let spine_len = spine_paths.len();
    let first_chapter_xhtml = if let Some(first) = spine_paths.first() {
        let chapter =
            read_entry(&mut archive, first).unwrap_or_else(|_| "<p>(empty chapter)</p>".into());
        inline_epub_images(&mut archive, first, &chapter)
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
    let mobi =
        mobi::Mobi::new(bytes.to_vec()).map_err(|e| Error::Parse(format!("mobi parse: {}", e)))?;

    let title = mobi.title().to_string();
    let author = mobi.author().map(|s| s.to_string());

    // Extract text content — MOBI uses PalmDOC compression
    // The mobi crate handles decompression internally
    let text = mobi
        .content_as_string()
        .unwrap_or_else(|_| mobi.content_as_string_lossy());
    let html = mobi_content_to_xhtml(&text);

    Ok(Document::Epub {
        title,
        author,
        first_chapter_xhtml: html,
        spine_len: 1, // MOBI is single-flow
        byte_len: bytes.len(),
    })
}

fn mobi_content_to_xhtml(text: &str) -> String {
    let text = text.trim_matches('\0').trim();
    if text.is_empty() {
        return mobi_html_document("<p>(no text content)</p>");
    }

    let body = if looks_like_html(text) {
        let body = extract_html_body(text).unwrap_or(text);
        format!(
            "<div class=\"mobi-flow\">{}</div>",
            strip_mobi_html_noise(body)
        )
    } else {
        format!(
            "<div class=\"mobi-flow\">{}</div>",
            escape_text_as_html(text)
        )
    };

    mobi_html_document(&body)
}

fn mobi_html_document(body: &str) -> String {
    format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><style>html,body{{margin:0;padding:0;background:#fff;color:#111;font:16px/1.55 system-ui,-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif;}}body{{padding:1rem;}}p{{margin:0 0 0.85rem;}}a{{color:#0b57d0;}}.mobi-flow{{max-width:72ch;margin:0 auto;}}img{{max-width:100%;height:auto;}}</style></head><body>{}</body></html>"#,
        body
    )
}

fn looks_like_html(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("<html")
        || lower.contains("<body")
        || lower.contains("<p")
        || lower.contains("<div")
}

fn extract_html_body(text: &str) -> Option<&str> {
    let lower = text.to_ascii_lowercase();
    let body_start = lower.find("<body")?;
    let after_open = lower[body_start..].find('>')? + body_start + 1;
    let body_end = lower[after_open..]
        .find("</body>")
        .map(|idx| after_open + idx)
        .unwrap_or(text.len());
    Some(&text[after_open..body_end])
}

fn strip_mobi_html_noise(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.to_ascii_lowercase().find("<guide") {
        out.push_str(&rest[..start]);
        let lower = rest.to_ascii_lowercase();
        if let Some(end) = lower[start..].find("</guide>") {
            rest = &rest[start + end + "</guide>".len()..];
        } else if let Some(end) = rest[start..].find('>') {
            rest = &rest[start + end + 1..];
        } else {
            rest = "";
        }
    }
    out.push_str(rest);
    out
}

fn escape_text_as_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\n', "<br>")
}

fn inline_epub_images<R>(archive: &mut ZipArchive<R>, chapter_path: &str, html: &str) -> String
where
    R: Read + std::io::Seek,
{
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some((attr_start, attr_len, quote, value_start)) = find_image_attr(rest) {
        out.push_str(&rest[..attr_start + attr_len]);
        let Some(value_end_rel) = rest[value_start..].find(quote) else {
            out.push_str(&rest[attr_start + attr_len..]);
            return out;
        };
        let value_end = value_start + value_end_rel;
        let src = &rest[value_start..value_end];
        out.push(quote);
        out.push_str(
            &image_data_url(archive, chapter_path, src).unwrap_or_else(|| src.to_string()),
        );
        out.push(quote);
        rest = &rest[value_end + quote.len_utf8()..];
    }
    out.push_str(rest);
    out
}

fn find_image_attr(text: &str) -> Option<(usize, usize, char, usize)> {
    let lower = text.to_ascii_lowercase();
    ["src=", "href=", "xlink:href="]
        .iter()
        .filter_map(|attr| lower.find(attr).map(|idx| (idx, attr.len())))
        .min_by_key(|(idx, _)| *idx)
        .and_then(|(idx, len)| {
            let quote_idx = idx + len;
            let quote = *text.as_bytes().get(quote_idx)? as char;
            if quote != '\'' && quote != '"' {
                return None;
            }
            Some((idx, len, quote, quote_idx + 1))
        })
}

fn image_data_url<R>(archive: &mut ZipArchive<R>, chapter_path: &str, src: &str) -> Option<String>
where
    R: Read + std::io::Seek,
{
    if src.starts_with("data:") || src.starts_with("http:") || src.starts_with("https:") {
        return None;
    }
    let src = src.split('#').next().unwrap_or(src);
    let path = normalize_relative_path(chapter_path, src);
    let mime = image_mime(&path)?;
    let mut file = archive.by_name(&path).ok()?;
    let mut bytes = Vec::with_capacity(file.size() as usize);
    file.read_to_end(&mut bytes).ok()?;
    Some(format!(
        "data:{};base64,{}",
        mime,
        general_purpose::STANDARD.encode(bytes)
    ))
}

fn normalize_relative_path(base_file: &str, rel: &str) -> String {
    let mut parts: Vec<&str> = base_file.split('/').collect();
    parts.pop();
    for part in rel.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            _ => parts.push(part),
        }
    }
    parts.join("/")
}

fn image_mime(path: &str) -> Option<&'static str> {
    match path.rsplit('.').next()?.to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => Some("image/jpeg"),
        "png" => Some("image/png"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "svg" => Some("image/svg+xml"),
        _ => None,
    }
}

fn read_entry<R>(archive: &mut ZipArchive<R>, name: &str) -> Result<String, Error>
where
    R: Read + std::io::Seek,
{
    let mut f = archive
        .by_name(name)
        .map_err(|e| Error::Parse(format!("zip entry '{}' not found: {}", name, e)))?;
    let mut s = String::new();
    f.read_to_string(&mut s)
        .map_err(|e| Error::Parse(format!("read: {}", e)))?;
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
                            opf_path =
                                Some(String::from_utf8_lossy(attr.value.as_ref()).into_owned());
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(Error::Parse(format!("container.xml parse: {}", e))),
            _ => (),
        }
        buf.clear();
    }
    opf_path.ok_or_else(|| Error::Parse("no <rootfile full-path=> in container.xml".into()))
}

/// Parse OPF (XML), return (title, author, ordered list of entry paths).
fn parse_opf(xml: &str, opf_path: &str) -> Result<(String, Option<String>, Vec<String>), Error> {
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
                                b"id" => {
                                    id_v = String::from_utf8_lossy(attr.value.as_ref()).into_owned()
                                }
                                b"href" => {
                                    href_v =
                                        String::from_utf8_lossy(attr.value.as_ref()).into_owned()
                                }
                                _ => {}
                            }
                        }
                        if !id_v.is_empty() {
                            manifest.insert(id_v, normalize_relative_path(opf_path, &href_v));
                        }
                    }
                    b"itemref" if in_spine_section => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"idref" {
                                spine.push(
                                    String::from_utf8_lossy(attr.value.as_ref()).into_owned(),
                                );
                            }
                        }
                    }
                    b"dc:title" if in_metadata => in_title_meta = true,
                    b"dc:creator" if in_metadata => in_author_meta = true,
                    _ => {}
                }
            }
            Ok(Event::End(e)) => match e.name().as_ref() {
                b"metadata" => in_metadata = false,
                b"manifest" => in_manifest_section = false,
                b"spine" => in_spine_section = false,
                b"dc:title" => in_title_meta = false,
                b"dc:creator" => in_author_meta = false,
                _ => {}
            },
            Ok(Event::Text(t)) => {
                let s = t
                    .unescape()
                    .map_err(|e| Error::Parse(format!("esc: {}", e)))?
                    .into_owned();
                if in_title_meta && title.is_empty() {
                    title = s;
                } else if in_author_meta && author.is_none() {
                    author = Some(s);
                }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A degenerate MOBI fixture cannot easily be constructed by hand, so this
    /// test only asserts that the parse path fails cleanly with a Parse error
    /// (not a panic) on bytes that are not a valid MOBI. The real MOBI path is
    /// covered by the device smoke (`sample.mobi`) and recorded in
    /// `.agent/failures/format-verification.md` after the lossy fallback fix.
    #[test]
    fn mobi_parse_invalid_bytes_returns_parse_error_not_panic() {
        let bytes = [0u8; 16];
        let result = parse_mobi(&bytes);
        assert!(
            matches!(result, Err(Error::Parse(_))),
            "invalid MOBI bytes should surface a Parse error, got {:?}",
            result
        );
    }

    #[test]
    fn mobi_html_flow_renders_as_html_not_escaped_text() {
        let html = mobi_content_to_xhtml(
            r#"<html><head><guide><reference type="toc" /></guide></head><body><p height="19em">Readable text</p></body></html>"#,
        );
        assert!(html.contains("<p height=\"19em\">Readable text</p>"));
        assert!(html.contains("<meta name=\"viewport\""));
        assert!(!html.contains("&lt;html"));
        assert!(!html.contains("<guide"));
    }

    #[test]
    fn epub_first_chapter_inlines_relative_svg_image() {
        let mut bytes = Vec::new();
        {
            let cursor = std::io::Cursor::new(&mut bytes);
            let mut zip = zip::ZipWriter::new(cursor);
            let opts = zip::write::SimpleFileOptions::default();
            zip.start_file("META-INF/container.xml", opts).unwrap();
            std::io::Write::write_all(
                &mut zip,
                br#"<container><rootfiles><rootfile full-path="content.opf"/></rootfiles></container>"#,
            )
            .unwrap();
            zip.start_file("content.opf", opts).unwrap();
            std::io::Write::write_all(
                &mut zip,
                br#"<package xmlns="http://www.idpf.org/2007/opf"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>T</dc:title></metadata><manifest><item id="cover" href="OEBPS/cover.xml"/></manifest><spine><itemref idref="cover"/></spine></package>"#,
            )
            .unwrap();
            zip.start_file("OEBPS/cover.xml", opts).unwrap();
            std::io::Write::write_all(
                &mut zip,
                br#"<html><body><svg><image xlink:href="images/cover.jpg"/></svg></body></html>"#,
            )
            .unwrap();
            zip.start_file("OEBPS/images/cover.jpg", opts).unwrap();
            std::io::Write::write_all(&mut zip, b"fake-jpeg").unwrap();
            zip.finish().unwrap();
        }

        let doc = parse_epub(&bytes).unwrap();
        let Document::Epub {
            first_chapter_xhtml,
            ..
        } = doc
        else {
            panic!("expected epub document")
        };
        assert!(first_chapter_xhtml.contains("data:image/jpeg;base64,"));
        assert!(!first_chapter_xhtml.contains("images/cover.jpg"));
    }

    #[test]
    fn epub_resolves_chapter_from_nested_opf_directory() {
        let mut bytes = Vec::new();
        {
            let cursor = std::io::Cursor::new(&mut bytes);
            let mut zip = zip::ZipWriter::new(cursor);
            let opts = zip::write::SimpleFileOptions::default();
            zip.start_file("META-INF/container.xml", opts).unwrap();
            std::io::Write::write_all(
                &mut zip,
                br#"<container><rootfiles><rootfile full-path="OPS/package/content.opf"/></rootfiles></container>"#,
            )
            .unwrap();
            zip.start_file("OPS/package/content.opf", opts).unwrap();
            std::io::Write::write_all(
                &mut zip,
                br#"<package><metadata><dc:title>Nested</dc:title></metadata><manifest><item id="chapter" href="../text/chapter.xhtml"/></manifest><spine><itemref idref="chapter"/></spine></package>"#,
            )
            .unwrap();
            zip.start_file("OPS/text/chapter.xhtml", opts).unwrap();
            std::io::Write::write_all(
                &mut zip,
                br#"<html><body><p>Nested chapter resolved</p><img src="../images/pixel.png"/></body></html>"#,
            )
            .unwrap();
            zip.start_file("OPS/images/pixel.png", opts).unwrap();
            std::io::Write::write_all(&mut zip, b"fake-png").unwrap();
            zip.finish().unwrap();
        }

        let doc = parse_epub(&bytes).unwrap();
        let Document::Epub {
            first_chapter_xhtml,
            ..
        } = doc
        else {
            panic!("expected epub document")
        };
        assert!(first_chapter_xhtml.contains("Nested chapter resolved"));
        assert!(first_chapter_xhtml.contains("data:image/png;base64,"));
    }
}
