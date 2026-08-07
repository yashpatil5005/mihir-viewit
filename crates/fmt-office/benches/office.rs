use std::io::{Cursor, Write};

use viewit_fmt_office::render;

/// Create a minimal valid DOCX (ZIP with word/document.xml)
fn minimal_docx() -> Vec<u8> {
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

/// Create a minimal valid XLSX (ZIP with xl/worksheets/sheet1.xml)
fn minimal_xlsx() -> Vec<u8> {
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

/// Create a minimal valid PPTX
fn minimal_pptx() -> Vec<u8> {
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
        zip.write_all(b"<?xml version=\"1.0\"?><p:sld xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\"><p:cSld><p:spTree><p:sp><p:nvSpPr><p:ph type=\"title\"/></p:nvSpPr><p:txBody><a:p><a:t>Slide Title</a:t></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:sld>").unwrap();
        zip.start_file("ppt/_rels/presentation.xml.rels", options)
            .unwrap();
        zip.write_all(b"<?xml version=\"1.0\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide\" Target=\"slides/slide1.xml\"/></Relationships>").unwrap();
        zip.finish().unwrap();
    }
    buf
}

/// Create a minimal valid ODT
fn minimal_odt() -> Vec<u8> {
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

fn bench_render(c: &mut criterion::Criterion, ext: &str, bytes: &[u8]) {
    let mut group = c.benchmark_group(format!("office/{ext}"));
    group.sample_size(200);
    group.bench_function("render", |b| {
        b.iter(|| {
            let out = render(criterion::black_box(bytes), ext).unwrap();
            criterion::black_box(out)
        })
    });
    group.finish();
}

fn render_docx(c: &mut criterion::Criterion) {
    bench_render(c, "docx", &minimal_docx());
}
fn render_xlsx(c: &mut criterion::Criterion) {
    bench_render(c, "xlsx", &minimal_xlsx());
}
fn render_pptx(c: &mut criterion::Criterion) {
    bench_render(c, "pptx", &minimal_pptx());
}
fn render_odt(c: &mut criterion::Criterion) {
    bench_render(c, "odt", &minimal_odt());
}

criterion::criterion_group!(benches, render_docx, render_xlsx, render_pptx, render_odt);
criterion::criterion_main!(benches);
