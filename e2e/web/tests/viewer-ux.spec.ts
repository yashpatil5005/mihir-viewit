import { expect, test, type Page } from "@playwright/test";
import JSZip from "jszip";

async function openFile(page: Page, name: string, mimeType: string, buffer: Buffer) {
  const chooserPromise = page.waitForEvent("filechooser");
  await page.getByRole("button", { name: "Open file" }).click();
  const chooser = await chooserPromise;
  await chooser.setFiles({ name, mimeType, buffer });
}

async function xlsxFixture(): Promise<Buffer> {
  const zip = new JSZip();
  zip.file(
    "[Content_Types].xml",
    `<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  <Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
  <Override PartName="/xl/worksheets/sheet2.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
</Types>`,
  );
  zip.file(
    "_rels/.rels",
    `<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>`,
  );
  zip.file(
    "xl/workbook.xml",
    `<?xml version="1.0" encoding="UTF-8"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets><sheet name="Summary" sheetId="1" r:id="rId1"/><sheet name="Details" sheetId="2" r:id="rId2"/></sheets>
</workbook>`,
  );
  zip.file(
    "xl/_rels/workbook.xml.rels",
    `<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet2.xml"/>
</Relationships>`,
  );
  const worksheet = (heading: string, value: string) => `<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>
  <row r="1"><c r="A1" t="inlineStr"><is><t>${heading}</t></is></c></row>
  <row r="2"><c r="A2" t="inlineStr"><is><t>${value}</t></is></c></row>
</sheetData></worksheet>`;
  zip.file(
    "xl/worksheets/sheet1.xml",
    worksheet("Metric", "A long selected value for touch users"),
  );
  zip.file("xl/worksheets/sheet2.xml", worksheet("Category", "Second sheet"));
  return zip.generateAsync({ type: "nodebuffer", compression: "DEFLATE" });
}

function pdfFixture(): Buffer {
  const objects = [
    "<< /Type /Catalog /Pages 2 0 R >>",
    "<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>",
    "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << >> /Contents 5 0 R >>",
    "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << >> /Contents 5 0 R >>",
    "<< /Length 0 >>\nstream\n\nendstream",
  ];
  let body = "%PDF-1.4\n";
  const offsets = [0];
  for (let index = 0; index < objects.length; index++) {
    offsets.push(Buffer.byteLength(body));
    body += `${index + 1} 0 obj\n${objects[index]}\nendobj\n`;
  }
  const xref = Buffer.byteLength(body);
  body += `xref\n0 ${objects.length + 1}\n0000000000 65535 f \n`;
  body += offsets
    .slice(1)
    .map((offset) => `${String(offset).padStart(10, "0")} 00000 n \n`)
    .join("");
  body += `trailer\n<< /Size ${objects.length + 1} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`;
  return Buffer.from(body);
}

function wavFixture(): Buffer {
  const samples = 800;
  const buffer = Buffer.alloc(44 + samples * 2);
  buffer.write("RIFF", 0);
  buffer.writeUInt32LE(buffer.length - 8, 4);
  buffer.write("WAVEfmt ", 8);
  buffer.writeUInt32LE(16, 16);
  buffer.writeUInt16LE(1, 20);
  buffer.writeUInt16LE(1, 22);
  buffer.writeUInt32LE(8000, 24);
  buffer.writeUInt32LE(16000, 28);
  buffer.writeUInt16LE(2, 32);
  buffer.writeUInt16LE(16, 34);
  buffer.write("data", 36);
  buffer.writeUInt32LE(samples * 2, 40);
  return buffer;
}

test("XLSX tabs and selected cells are keyboard and touch accessible", async ({ page }) => {
  await page.goto("/");
  await openFile(
    page,
    "sheets.xlsx",
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    await xlsxFixture(),
  );

  const tabs = page.getByRole("tab");
  await expect(tabs).toHaveCount(2);
  await expect(tabs.first()).toHaveAttribute("aria-selected", "true");
  await tabs.first().focus();
  await page.keyboard.press("ArrowRight");
  await expect(tabs.nth(1)).toBeFocused();
  await expect(tabs.nth(1)).toHaveAttribute("aria-selected", "true");

  await tabs.first().click();
  const cell = page.getByRole("button", { name: /Cell A2:/ });
  await cell.click();
  await expect(page.getByRole("status")).toContainText("A long selected value for touch users");
  const close = page.getByRole("button", { name: "Close cell detail" });
  const box = await close.boundingBox();
  expect(box?.width).toBeGreaterThanOrEqual(44);
  expect(box?.height).toBeGreaterThanOrEqual(44);
  await expect(page).toHaveScreenshot("xlsx-cell-detail-desktop.png", {
    animations: "disabled",
    fullPage: true,
  });
});

test("PDF thumbnails clear the sticky header and PDF keys ignore controls", async ({ page }) => {
  await page.goto("/");
  await openFile(page, "pages.pdf", "application/pdf", pdfFixture());
  const thumbnails = page.locator(".thumbnail");
  await expect(thumbnails).toHaveCount(2, { timeout: 10_000 });

  const geometry = await page.evaluate(() => {
    const header = document.querySelector<HTMLElement>(".app-header")!;
    const sidebar = document.querySelector<HTMLElement>(".thumbnail-sidebar")!;
    const thumbnail = document.querySelector<HTMLElement>(".thumbnail")!;
    return {
      headerBottom: header.getBoundingClientRect().bottom,
      sidebarTop: sidebar.getBoundingClientRect().top,
      thumbnailWidth: thumbnail.offsetWidth,
      thumbnailHeight: thumbnail.offsetHeight,
    };
  });
  expect(geometry.sidebarTop).toBeGreaterThanOrEqual(geometry.headerBottom - 1);
  expect(geometry.thumbnailWidth).toBeGreaterThanOrEqual(44);
  expect(geometry.thumbnailHeight).toBeGreaterThanOrEqual(44);

  await page.evaluate(() => {
    const input = document.createElement("input");
    input.setAttribute("aria-label", "PDF test input");
    document.querySelector(".pdf-viewer")?.prepend(input);
    input.focus();
  });
  const before = await page.evaluate(() => scrollY);
  await page.keyboard.press("ArrowDown");
  expect(await page.evaluate(() => scrollY)).toBe(before);
  await expect(page).toHaveScreenshot("pdf-thumbnails-desktop.png", {
    animations: "disabled",
    fullPage: true,
  });
});

test("media controls do not overflow at 320px", async ({ page }) => {
  await page.setViewportSize({ width: 320, height: 720 });
  await page.goto("/");
  await openFile(page, "silence.wav", "audio/wav", wavFixture());
  await expect(page.locator(".controls")).toBeVisible();

  const geometry = await page.evaluate(() => ({
    viewportWidth: document.documentElement.clientWidth,
    scrollWidth: document.documentElement.scrollWidth,
    targets: [
      ...document.querySelectorAll<HTMLElement>(
        ".controls button, .controls input, .controls select",
      ),
    ].map((element) => ({ width: element.offsetWidth, height: element.offsetHeight })),
  }));
  expect(geometry.scrollWidth).toBeLessThanOrEqual(geometry.viewportWidth);
  for (const target of geometry.targets) {
    expect(Math.max(target.width, target.height)).toBeGreaterThanOrEqual(44);
  }
  await expect(page).toHaveScreenshot("media-controls-phone-narrow.png", {
    animations: "disabled",
    fullPage: true,
  });
});

test("presentation stage remains 16:9 on portrait and ultrawide screens", async ({ page }) => {
  const zip = new JSZip();
  zip.file("Data/slide.txt", "Container-relative presentation text\n");
  const fixture = await zip.generateAsync({ type: "nodebuffer", compression: "DEFLATE" });

  for (const viewport of [
    { width: 320, height: 720 },
    { width: 1920, height: 1080 },
  ]) {
    await page.setViewportSize(viewport);
    await page.goto("/");
    await openFile(page, "sample.key", "application/zip", fixture);
    const stage = page.locator(".stage");
    await expect(stage).toBeVisible();
    const box = await stage.boundingBox();
    expect(box).not.toBeNull();
    expect(box!.width / box!.height).toBeCloseTo(16 / 9, 1);
    expect(box!.width).toBeLessThanOrEqual(viewport.width);
    await expect(page).toHaveScreenshot(
      `presentation-${viewport.width === 320 ? "portrait" : "ultrawide"}.png`,
      { animations: "disabled", fullPage: true },
    );
  }
});
