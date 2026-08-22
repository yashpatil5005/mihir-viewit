import fs from "node:fs";
import path from "node:path";
import JSZip from "jszip";

const FIXTURES_DIR = path.resolve("build/desktop-fixtures");
fs.mkdirSync(FIXTURES_DIR, { recursive: true });

async function createZip(name, files) {
  const zip = new JSZip();
  for (const [p, content] of Object.entries(files)) {
    zip.file(p, content);
  }
  const buf = await zip.generateAsync({ type: "nodebuffer", compression: "DEFLATE" });
  fs.writeFileSync(path.join(FIXTURES_DIR, name), buf);
}

// 1. Plain Text
fs.writeFileSync(
  path.join(FIXTURES_DIR, "sample.txt"),
  "Hello, ViewIt Offline Desktop!\nLine 2 of test text.\n",
);

// 2. Markdown
fs.writeFileSync(
  path.join(FIXTURES_DIR, "sample.md"),
  "# ViewIt Offline Markdown\n\n- Item 1\n- Item 2\n\n**Bold text** and *italic*.\n",
);

// 3. JSON
fs.writeFileSync(
  path.join(FIXTURES_DIR, "sample.json"),
  JSON.stringify({ app: "ViewIt", status: "offline", count: 42 }, null, 2),
);

// 4. CSV
fs.writeFileSync(
  path.join(FIXTURES_DIR, "sample.csv"),
  "Name,Role,Status\nAlice,Developer,Active\nBob,Designer,Offline\n",
);

// 5. XML
fs.writeFileSync(
  path.join(FIXTURES_DIR, "sample.xml"),
  '<?xml version="1.0" encoding="UTF-8"?>\n<root><item id="1">Offline XML</item></root>\n',
);

// 6. HTML
fs.writeFileSync(
  path.join(FIXTURES_DIR, "sample.html"),
  "<!DOCTYPE html><html><body><h1>Desktop HTML Viewer</h1><p>Offline paragraph.</p></body></html>\n",
);

// 7. VCF
fs.writeFileSync(
  path.join(FIXTURES_DIR, "sample.vcf"),
  "BEGIN:VCARD\nVERSION:3.0\nN:Desktop;Tester;;;\nFN:Tester Desktop\nTEL;TYPE=CELL:1234567890\nEND:VCARD\n",
);

// 8. ICS
fs.writeFileSync(
  path.join(FIXTURES_DIR, "sample.ics"),
  "BEGIN:VCALENDAR\nVERSION:2.0\nBEGIN:VEVENT\nSUMMARY:Desktop Release Testing\nDTSTART:20260821T100000Z\nDTEND:20260821T110000Z\nEND:VEVENT\nEND:VCALENDAR\n",
);

// 9. .desktop entry
fs.writeFileSync(
  path.join(FIXTURES_DIR, "sample.desktop"),
  "[Desktop Entry]\nType=Application\nName=ViewIt Desktop\nExec=viewit-desktop\nIcon=viewit\nCategories=Utility;\n",
);

// 10. Minimal WAV
const wavHeader = Buffer.alloc(44 + 1600);
wavHeader.write("RIFF", 0);
wavHeader.writeUInt32LE(wavHeader.length - 8, 4);
wavHeader.write("WAVEfmt ", 8);
wavHeader.writeUInt32LE(16, 16);
wavHeader.writeUInt16LE(1, 20);
wavHeader.writeUInt16LE(1, 22);
wavHeader.writeUInt32LE(8000, 24);
wavHeader.writeUInt32LE(16000, 28);
wavHeader.writeUInt16LE(2, 32);
wavHeader.writeUInt16LE(16, 34);
wavHeader.write("data", 36);
wavHeader.writeUInt32LE(1600, 40);
fs.writeFileSync(path.join(FIXTURES_DIR, "sample.wav"), wavHeader);

// 11. Minimal PDF
const objects = [
  "<< /Type /Catalog /Pages 2 0 R >>",
  "<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>",
  "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << >> /Contents 5 0 R >>",
  "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << >> /Contents 5 0 R >>",
  "<< /Length 0 >>\nstream\n\nendstream",
];
let pdfBody = "%PDF-1.4\n";
const offsets = [0];
for (let i = 0; i < objects.length; i++) {
  offsets.push(Buffer.byteLength(pdfBody));
  pdfBody += `${i + 1} 0 obj\n${objects[i]}\nendobj\n`;
}
const xref = Buffer.byteLength(pdfBody);
pdfBody += `xref\n0 ${objects.length + 1}\n0000000000 65535 f \n`;
pdfBody += offsets
  .slice(1)
  .map((o) => `${String(o).padStart(10, "0")} 00000 n \n`)
  .join("");
pdfBody += `trailer\n<< /Size ${objects.length + 1} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`;
fs.writeFileSync(path.join(FIXTURES_DIR, "sample.pdf"), Buffer.from(pdfBody));

// 12. Minimal PNG (1x1 transparent)
const pngBase64 =
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";
fs.writeFileSync(path.join(FIXTURES_DIR, "sample.png"), Buffer.from(pngBase64, "base64"));

// 13. Minimal JPG (1x1 black)
const jpgBase64 =
  "/9j/4AAQSkZJRgABAQEASABIAAD/2wBDAP//////////////////////////////////////////////////////////////////////////////////////wgALCAABAAEBAREA/8QAFBABAAAAAAAAAAAAAAAAAAAAAP/aAAgBAQABPxA=";
fs.writeFileSync(path.join(FIXTURES_DIR, "sample.jpg"), Buffer.from(jpgBase64, "base64"));

// 14. ZIP archive
await createZip("sample.zip", {
  "hello.txt": "Hello from inside ZIP archive!\n",
  "folder/nested.txt": "Nested text file content\n",
});

// 15. XLSX spreadsheet
await createZip("sample.xlsx", {
  "[Content_Types].xml": `<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/></Types>`,
  "_rels/.rels": `<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>`,
  "xl/workbook.xml": `<?xml version="1.0" encoding="UTF-8"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Sheet1" sheetId="1" r:id="rId1"/></sheets></workbook>`,
  "xl/_rels/workbook.xml.rels": `<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/></Relationships>`,
  "xl/worksheets/sheet1.xml": `<?xml version="1.0" encoding="UTF-8"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>Desktop Product</t></is></c><c r="B1" t="inlineStr"><is><t>Revenue</t></is></c></row><row r="2"><c r="A2" t="inlineStr"><is><t>ViewIt Native</t></is></c><c r="B2" t="inlineStr"><is><t>1000000</t></is></c></row></sheetData></worksheet>`,
});

// 16. DOCX document
await createZip("sample.docx", {
  "[Content_Types].xml": `<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>`,
  "_rels/.rels": `<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>`,
  "word/document.xml": `<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Hello from Offline Desktop DOCX Document</w:t></w:r></w:p></w:body></w:document>`,
});

// 17. PPTX presentation
await createZip("sample.pptx", {
  "[Content_Types].xml": `<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/><Override PartName="/ppt/slides/slide1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/></Types>`,
  "_rels/.rels": `<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/></Relationships>`,
  "ppt/presentation.xml": `<?xml version="1.0" encoding="UTF-8"?><p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><p:sldIdLst><p:sldId id="256" r:id="rId1"/></p:sldIdLst></p:presentation>`,
  "ppt/_rels/presentation.xml.rels": `<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide1.xml"/></Relationships>`,
  "ppt/slides/slide1.xml": `<?xml version="1.0" encoding="UTF-8"?><p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/><p:sp><p:nvSpPr><p:cNvPr id="2" name="Title"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>Offline Desktop Slide Deck</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:sld>`,
});

// 18. ODT document
await createZip("sample.odt", {
  mimetype: "application/vnd.oasis.opendocument.text",
  "content.xml": `<?xml version="1.0" encoding="UTF-8"?><office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"><office:body><office:text><text:p>Offline OpenDocument Text Content</text:p></office:text></office:body></office:document-content>`,
});

// 19. ODS spreadsheet
await createZip("sample.ods", {
  mimetype: "application/vnd.oasis.opendocument.spreadsheet",
  "content.xml": `<?xml version="1.0" encoding="UTF-8"?><office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"><office:body><office:spreadsheet><table:table table:name="Sheet1"><table:table-row><table:table-cell><text:p>ODS Cell A1</text:p></table:table-cell></table:table-row></table:table></office:spreadsheet></office:body></office:document-content>`,
});

// 20. ODP presentation
await createZip("sample.odp", {
  mimetype: "application/vnd.oasis.opendocument.presentation",
  "content.xml": `<?xml version="1.0" encoding="UTF-8"?><office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"><office:body><office:presentation><draw:page draw:name="page1"><draw:frame><draw:text-box><text:p>ODP Slide Title</text:p></draw:text-box></draw:frame></draw:page></office:presentation></office:body></office:document-content>`,
});

// 21. EPUB book
await createZip("sample.epub", {
  mimetype: "application/epub+zip",
  "META-INF/container.xml": `<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>`,
  "OEBPS/content.opf": `<?xml version="1.0" encoding="UTF-8"?><package xmlns="http://www.idpf.org/2007/opf" unique-identifier="BookId" version="2.0"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Offline Desktop EPUB</dc:title></metadata><manifest><item id="chap1" href="chap1.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="chap1"/></spine></package>`,
  "OEBPS/chap1.xhtml": `<?xml version="1.0" encoding="UTF-8"?><html xmlns="http://www.w3.org/1999/xhtml"><body><h1>Chapter 1: Offline Desktop</h1><p>Reading real EPUB on desktop native.</p></body></html>`,
});

// 22. iWork Pages
await createZip("sample.pages", {
  "Data/document.txt": "Apple Pages offline desktop extract content.\n",
});

// 23. iWork Numbers
await createZip("sample.numbers", {
  "Data/table.csv": "Quarter,Revenue\nQ1,50000\nQ2,75000\n",
});

// 24. iWork Keynote
await createZip("sample.key", {
  "Data/slide.txt": "Keynote Presentation Slide 1 Title\n",
});

// 25. Font (copied from local node_modules — no network access).
const fontSource = path.resolve(
  "node_modules/pdfjs-dist/standard_fonts/LiberationSans-Regular.ttf",
);
if (fs.existsSync(fontSource)) {
  fs.copyFileSync(fontSource, path.join(FIXTURES_DIR, "sample.ttf"));
}

// 26. Deterministic multi-MB stress fixture (shared with web/device E2E via
// scripts/stress-fixture.mjs) — exercises the frozen-chrome guarantees.
const { makeStressText, STRESS_FILE_NAME } = await import("./stress-fixture.mjs");
fs.writeFileSync(path.join(FIXTURES_DIR, STRESS_FILE_NAME), makeStressText({ megabytes: 6 }));

console.log("Desktop offline fixtures generated successfully!");
