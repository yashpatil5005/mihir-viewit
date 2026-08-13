//! DOCX parser (docx, docm, dotx, dotm) — unified for office-universal.
//!
//! Uses docx-rust + WordprocessingML fallback.
//! Produces Document::Docx with structured blocks.

use std::io::Cursor;
use viewit_core_types::{Document, DocxBlock, DocxCell, DocxInline, DocxInlineImage, Error};

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
                let numbering = p.property.as_ref().and_then(|prop| prop.numbering.as_ref());
                let is_list = numbering
                    .and_then(|n| n.id.as_ref())
                    .is_some_and(|id| id.value != 0);
                let level = numbering
                    .and_then(|n| n.level.as_ref())
                    .map(|level| level.value.clamp(0, u8::MAX as isize) as u8)
                    .unwrap_or(0);
                push_docx_inlines_block(
                    &mut blocks,
                    paragraph_inlines(p, &rels),
                    heading,
                    is_list,
                    level,
                );
            }
            docx_rust::document::BodyContent::Table(t) => {
                let mut rows: Vec<Vec<DocxCell>> = Vec::new();
                let mut rich = false;
                for row in &t.rows {
                    let mut cells = Vec::new();
                    for tc in &row.cells {
                        let cell = match tc {
                            docx_rust::document::TableRowContent::TableCell(c) => c,
                            _ => continue,
                        };
                        let mut inlines = Vec::new();
                        for content in &cell.content {
                            let docx_rust::document::TableCellContent::Paragraph(p) = content;
                            if !inlines.is_empty() {
                                inlines.push(DocxInline {
                                    text: "\n".into(),
                                    ..Default::default()
                                });
                            }
                            inlines.extend(paragraph_inlines(p, &rels));
                        }
                        rich |= inlines.iter().any(inline_has_rich_detail);
                        cells.push(DocxCell { inlines });
                    }
                    rows.push(cells);
                }
                if rich {
                    let plain_rows = rows
                        .iter()
                        .map(|row| row.iter().map(|cell| inline_text(&cell.inlines)).collect())
                        .collect();
                    blocks.push(DocxBlock::Table {
                        rows: plain_rows,
                        rich_rows: Some(rows),
                    });
                } else {
                    blocks.push(DocxBlock::Table {
                        rows: rows
                            .into_iter()
                            .map(|row| {
                                row.into_iter()
                                    .map(|cell| inline_text(&cell.inlines))
                                    .collect()
                            })
                            .collect(),
                        rich_rows: None,
                    });
                }
            }
            _ => {}
        }
    }

    Ok(Document::Docx {
        blocks,
        byte_len: bytes.len(),
    })
}

fn push_docx_inlines_block(
    blocks: &mut Vec<DocxBlock>,
    inlines: Vec<DocxInline>,
    heading: Option<u8>,
    is_list: bool,
    level: u8,
) {
    let text = inline_text(&inlines);
    if text.trim().is_empty() && !inlines.iter().any(|inline| inline.image.is_some()) {
        return;
    }
    let rich = inlines.iter().any(inline_has_rich_detail);
    if is_list && rich {
        blocks.push(DocxBlock::ListItem {
            text,
            level,
            inlines: Some(inlines),
        });
    } else if is_list {
        blocks.push(DocxBlock::ListItem {
            text,
            level,
            inlines: None,
        });
    } else if rich {
        blocks.push(DocxBlock::Paragraph {
            text,
            heading,
            inlines: Some(inlines),
        });
    } else {
        blocks.push(DocxBlock::Paragraph {
            text,
            heading,
            inlines: None,
        });
    }
}

fn paragraph_inlines(
    paragraph: &docx_rust::document::Paragraph<'_>,
    rels: &std::collections::HashMap<String, String>,
) -> Vec<DocxInline> {
    use docx_rust::document::ParagraphContent;
    let mut inlines = Vec::new();
    for content in &paragraph.content {
        match content {
            ParagraphContent::Run(run) => append_run_inlines(&mut inlines, run, None, rels),
            ParagraphContent::Link(link) => {
                let href = link
                    .id
                    .as_deref()
                    .and_then(|id| rels.get(id))
                    .cloned()
                    .or_else(|| link.anchor.as_ref().map(|anchor| format!("#{anchor}")));
                if let Some(run) = &link.content {
                    append_run_inlines(&mut inlines, run, href, rels);
                }
            }
            ParagraphContent::SDT(sdt) => {
                let text: String = sdt.iter_text().map(|text| text.as_ref()).collect();
                if !text.is_empty() {
                    inlines.push(DocxInline {
                        text,
                        ..Default::default()
                    });
                }
            }
            _ => {}
        }
    }
    inlines
}

fn append_run_inlines(
    inlines: &mut Vec<DocxInline>,
    run: &docx_rust::document::Run<'_>,
    href: Option<String>,
    rels: &std::collections::HashMap<String, String>,
) {
    use docx_rust::document::RunContent;
    let property = run.property.as_ref();
    let styled = |text: String, image: Option<DocxInlineImage>| DocxInline {
        text,
        bold: property
            .and_then(|p| p.bold.as_ref())
            .is_some_and(|value| value.value.unwrap_or(true)),
        italic: property
            .and_then(|p| p.italics.as_ref())
            .is_some_and(|value| value.value.unwrap_or(true)),
        underline: property
            .and_then(|p| p.underline.as_ref())
            .is_some_and(|value| {
                !matches!(value.val, Some(docx_rust::formatting::UnderlineStyle::None))
            }),
        color: property.and_then(|p| p.color.as_ref()).and_then(|color| {
            let value = color.value.as_ref();
            (value.len() == 6 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
                .then(|| format!("#{value}"))
        }),
        font_size: property
            .and_then(|p| p.size.as_ref())
            .map(|size| size.value as f64 / 2.0),
        href: href.clone(),
        image,
    };
    for content in &run.content {
        match content {
            RunContent::Text(text) => inlines.push(styled(text.text.to_string(), None)),
            RunContent::InstrText(_) => {}
            RunContent::Tab(_) => inlines.push(styled("\t".into(), None)),
            RunContent::Break(_) | RunContent::CarriageReturn(_) => {
                inlines.push(styled("\n".into(), None))
            }
            RunContent::Drawing(drawing) => {
                if let Some(name) = drawing_media_name(drawing, rels) {
                    inlines.push(styled(
                        String::new(),
                        Some(DocxInlineImage { name, src: None }),
                    ));
                }
            }
            _ => {}
        }
    }
}

fn inline_has_rich_detail(inline: &DocxInline) -> bool {
    inline.bold
        || inline.italic
        || inline.underline
        || inline.color.is_some()
        || inline.font_size.is_some()
        || inline.href.is_some()
        || inline.image.is_some()
}

fn inline_text(inlines: &[DocxInline]) -> String {
    inlines.iter().map(|inline| inline.text.as_str()).collect()
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
    let mut paragraph_list_level = 0;
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
                        paragraph_list_level = 0;
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
                    b"w:ilvl" if in_num_pr => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"w:val" {
                                paragraph_list_level = String::from_utf8_lossy(attr.value.as_ref())
                                    .trim()
                                    .parse::<u8>()
                                    .unwrap_or(0);
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
                                    blocks.push(DocxBlock::ListItem {
                                        text,
                                        level: paragraph_list_level,
                                        inlines: None,
                                    });
                                } else {
                                    blocks.push(DocxBlock::Paragraph {
                                        text,
                                        heading: paragraph_heading,
                                        inlines: None,
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
                                rich_rows: None,
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
                } else if e.name().as_ref() == b"w:ilvl" && in_num_pr {
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"w:val" {
                            paragraph_list_level = String::from_utf8_lossy(attr.value.as_ref())
                                .trim()
                                .parse::<u8>()
                                .unwrap_or(0);
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
    fn preserves_external_hyperlink_in_paragraph_order() {
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
        let DocxBlock::Paragraph {
            inlines: Some(inlines),
            ..
        } = &blocks[0]
        else {
            panic!("expected rich paragraph")
        };
        assert_eq!(inline_text(inlines), "Before ViewIt after");
        assert_eq!(inlines.len(), 3);
        assert_eq!(inlines[1].href.as_deref(), Some("https://viewit.example"));
    }

    #[test]
    fn preserves_run_styles_internal_links_and_nested_list_level() {
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
            zip.write_all(br#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body><w:p><w:pPr><w:numPr><w:ilvl w:val="2"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:rPr><w:b/><w:i/><w:u w:val="single"/><w:color w:val="12ABef"/><w:sz w:val="28"/></w:rPr><w:t>Styled</w:t></w:r><w:hyperlink w:anchor="target"><w:r><w:t> jump</w:t></w:r></w:hyperlink></w:p></w:body></w:document>"#).unwrap();
            zip.finish().unwrap();
        }
        let Document::Docx { blocks, .. } = parse_docx(&bytes).unwrap() else {
            panic!("expected Docx")
        };
        let DocxBlock::ListItem {
            inlines: Some(inlines),
            level,
            ..
        } = &blocks[0]
        else {
            panic!("expected rich list item")
        };
        assert_eq!(*level, 2);
        assert!(inlines[0].bold && inlines[0].italic && inlines[0].underline);
        assert_eq!(inlines[0].color.as_deref(), Some("#12ABef"));
        assert_eq!(inlines[0].font_size, Some(14.0));
        assert_eq!(inlines[1].href.as_deref(), Some("#target"));
    }

    #[test]
    fn rich_table_cells_preserve_links_and_styles() {
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
            zip.write_all(br#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tblGrid><w:gridCol w:w="2400"/></w:tblGrid><w:tr><w:tc><w:p><w:r><w:rPr><w:b/></w:rPr><w:t>Bold</w:t></w:r><w:hyperlink w:anchor="cell"><w:r><w:t> link</w:t></w:r></w:hyperlink></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#).unwrap();
            zip.finish().unwrap();
        }
        let Document::Docx { blocks, .. } = parse_docx(&bytes).unwrap() else {
            panic!("expected Docx")
        };
        let DocxBlock::Table {
            rich_rows: Some(rows),
            ..
        } = &blocks[0]
        else {
            panic!("expected rich table")
        };
        assert!(rows[0][0].inlines[0].bold);
        assert_eq!(rows[0][0].inlines[1].href.as_deref(), Some("#cell"));
    }

    #[test]
    fn image_only_paragraph_remains_inline() {
        let mut blocks = Vec::new();
        push_docx_inlines_block(
            &mut blocks,
            vec![DocxInline {
                image: Some(DocxInlineImage {
                    name: "image1.png".into(),
                    src: None,
                }),
                ..Default::default()
            }],
            None,
            false,
            0,
        );
        assert!(matches!(
            &blocks[0],
            DocxBlock::Paragraph { inlines: Some(inlines), .. }
                if inlines[0].image.as_ref().is_some_and(|image| image.name == "image1.png")
        ));
    }
}
