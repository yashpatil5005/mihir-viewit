//! DOCX parser (docx, docm, dotx, dotm) — unified for office-universal.
//!
//! Uses docx-rust + WordprocessingML fallback.
//! Produces Document::Docx with structured blocks.

use std::io::Cursor;
use viewit_core_types::{Document, DocxBlock, Error};

pub fn parse_docx(bytes: &[u8]) -> Result<Document, Error> {
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

    // Read document relationships to resolve image rIds → media filenames.
    let rels = docx_rels(bytes).unwrap_or_default();

    let mut blocks: Vec<DocxBlock> = Vec::new();
    for content in &docx.document.body.content {
        match content {
            docx_rust::document::BodyContent::Paragraph(p) => {
                let heading = p
                    .property
                    .as_ref()
                    .and_then(|prop| prop.style_id.as_ref())
                    .and_then(|sid| heading_from_style(&sid.value));
                let is_list = p
                    .property
                    .as_ref()
                    .and_then(|prop| prop.numbering.as_ref())
                    .and_then(|n| n.id.as_ref())
                    .map(|id| id.value != 0)
                    .unwrap_or(false);
                let mut pending_text = String::new();
                for pc in &p.content {
                    match pc {
                        docx_rust::document::ParagraphContent::Run(r) => {
                            pending_text.extend(r.iter_text().map(|text| text.as_ref()));
                            for rc in &r.content {
                                if let docx_rust::document::RunContent::Drawing(d) = rc {
                                    if let Some(name) = drawing_media_name(d, &rels) {
                                        push_docx_text_block(
                                            &mut blocks,
                                            std::mem::take(&mut pending_text),
                                            heading,
                                            is_list,
                                        );
                                        blocks.push(DocxBlock::Image { name, src: None });
                                    }
                                }
                            }
                        }
                        docx_rust::document::ParagraphContent::Link(link) => {
                            let text = link.text();
                            let href = link
                                .id
                                .as_deref()
                                .and_then(|id| rels.get(id))
                                .cloned()
                                .or_else(|| link.anchor.as_ref().map(|a| format!("#{a}")));
                            if !text.trim().is_empty() {
                                if let Some(href) = href {
                                    push_docx_text_block(
                                        &mut blocks,
                                        std::mem::take(&mut pending_text),
                                        heading,
                                        is_list,
                                    );
                                    blocks.push(DocxBlock::Hyperlink { text, href });
                                }
                            }
                        }
                        docx_rust::document::ParagraphContent::SDT(sdt) => {
                            pending_text.extend(sdt.iter_text().map(|text| text.as_ref()));
                        }
                        _ => {}
                    }
                }
                push_docx_text_block(&mut blocks, pending_text, heading, is_list);
            }
            docx_rust::document::BodyContent::Table(t) => {
                let mut rows: Vec<Vec<String>> = Vec::new();
                for row in &t.rows {
                    let mut cells: Vec<String> = Vec::new();
                    for tc in &row.cells {
                        let cell = match tc {
                            docx_rust::document::TableRowContent::TableCell(c) => c,
                            _ => continue,
                        };
                        let cell_text: Vec<String> = cell
                            .content
                            .iter()
                            .map(|c| match c {
                                docx_rust::document::TableCellContent::Paragraph(p) => p.text(),
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

fn push_docx_text_block(
    blocks: &mut Vec<DocxBlock>,
    text: String,
    heading: Option<u8>,
    is_list: bool,
) {
    if text.trim().is_empty() {
        return;
    }
    if is_list {
        blocks.push(DocxBlock::ListItem { text, level: 0 });
    } else {
        blocks.push(DocxBlock::Paragraph { text, heading });
    }
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
    let mut in_num_pr = false;
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
                    b"w:numPr" if in_paragraph => {
                        in_num_pr = true;
                        // Default: assume list unless numId=0 (no list) is found inside.
                        paragraph_is_list = true;
                    }
                    b"w:numId" if in_num_pr => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"w:val" {
                                let val = String::from_utf8_lossy(attr.value.as_ref());
                                if val.trim() == "0" {
                                    // numId=0 explicitly disables list formatting
                                    // (often inherited from a style). This is NOT a list item.
                                    paragraph_is_list = false;
                                }
                            }
                        }
                    }
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
                    b"w:numPr" => in_num_pr = false,
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
            Ok(Event::Empty(e)) => {
                // Self-closing tags (e.g. <w:numId w:val="0"/>).
                if e.name().as_ref() == b"w:numId" && in_num_pr {
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"w:val" {
                            let val = String::from_utf8_lossy(attr.value.as_ref());
                            if val.trim() == "0" {
                                paragraph_is_list = false;
                            }
                        }
                    }
                }
            }
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

/// Read `word/_rels/document.xml.rels` and map rId → Target (e.g.
/// "rId4" → "media/image1.png").
fn docx_rels(bytes: &[u8]) -> Option<std::collections::HashMap<String, String>> {
    use quick_xml::events::Event;
    use quick_xml::Reader;
    use std::io::{Cursor, Read};

    let cursor = Cursor::new(bytes.to_vec());
    let mut archive = zip::ZipArchive::new(cursor).ok()?;
    let mut rels_xml = String::new();
    archive
        .by_name("word/_rels/document.xml.rels")
        .ok()?
        .read_to_string(&mut rels_xml)
        .ok()?;

    let mut reader = Reader::from_str(&rels_xml);
    let mut buf = Vec::new();
    let mut map = std::collections::HashMap::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(e)) | Ok(Event::Start(e)) => {
                if e.name().as_ref() == b"Relationship" {
                    let mut id = String::new();
                    let mut target = String::new();
                    for attr in e.attributes().flatten() {
                        match attr.key.as_ref() {
                            b"Id" => id = String::from_utf8_lossy(attr.value.as_ref()).to_string(),
                            b"Target" => {
                                target = String::from_utf8_lossy(attr.value.as_ref()).to_string()
                            }
                            _ => {}
                        }
                    }
                    if !id.is_empty() && !target.is_empty() {
                        map.insert(id, target);
                    }
                }
            }
            Ok(Event::Eof) => break,
            _ => {}
        }
        buf.clear();
    }
    Some(map)
}

/// Extract the media filename from a `w:drawing` element via the blip's
/// `r:embed` relationship ID, resolved through the document rels map.
fn drawing_media_name(
    drawing: &docx_rust::document::Drawing,
    rels: &std::collections::HashMap<String, String>,
) -> Option<String> {
    let graphic = if let Some(inline) = &drawing.inline {
        inline.graphic.as_ref()
    } else if let Some(anchor) = &drawing.anchor {
        anchor.graphic.as_ref()
    } else {
        None
    }?;
    let pic = graphic.data.children.first()?;
    let embed = &pic.fill.blip.embed;
    if embed.is_empty() {
        return None;
    }
    let target = rels.get(embed.as_ref())?;
    let name = target.rsplit('/').next().unwrap_or(target);
    Some(name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heading_from_style_parses() {
        assert_eq!(heading_from_style("Title"), Some(1));
        assert_eq!(heading_from_style("Heading1"), Some(1));
        assert_eq!(heading_from_style("Heading 2"), Some(2));
        assert_eq!(heading_from_style("heading3"), Some(3));
        assert_eq!(heading_from_style("Normal"), None);
    }

    #[test]
    fn extracts_external_hyperlink_without_duplicate_paragraph() {
        use std::io::Write;
        let mut bytes = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut bytes));
            let options = zip::write::SimpleFileOptions::default();
            zip.start_file("[Content_Types].xml", options).unwrap();
            zip.write_all(br#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#).unwrap();
            zip.start_file("_rels/.rels", options).unwrap();
            zip.write_all(br#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#).unwrap();
            zip.start_file("word/document.xml", options).unwrap();
            zip.write_all(br#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body><w:p><w:r><w:t>Before </w:t></w:r><w:hyperlink r:id="rId1"><w:r><w:t>ViewIt</w:t></w:r></w:hyperlink><w:r><w:t> after</w:t></w:r></w:p></w:body></w:document>"#).unwrap();
            zip.start_file("word/_rels/document.xml.rels", options)
                .unwrap();
            zip.write_all(br#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://viewit.example" TargetMode="External"/></Relationships>"#).unwrap();
            zip.finish().unwrap();
        }
        let Document::Docx { blocks, .. } = parse_docx(&bytes).unwrap() else {
            panic!("expected Docx")
        };
        assert!(matches!(&blocks[0], DocxBlock::Paragraph { text, .. } if text == "Before "));
        assert!(
            matches!(&blocks[1], DocxBlock::Hyperlink { text, href } if text == "ViewIt" && href == "https://viewit.example")
        );
        assert!(matches!(&blocks[2], DocxBlock::Paragraph { text, .. } if text == " after"));
    }
}
