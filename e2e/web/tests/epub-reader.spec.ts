import { expect, test, type Page } from "@playwright/test";
import JSZip from "jszip";

async function epubFixture(): Promise<Buffer> {
  const zip = new JSZip();
  zip.file("mimetype", "application/epub+zip", { compression: "STORE" });
  zip.file(
    "META-INF/container.xml",
    `<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>`,
  );
  zip.file(
    "OPS/content.opf",
    `<?xml version="1.0"?><package version="2.0" xmlns="http://www.idpf.org/2007/opf" unique-identifier="id"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Interaction Test Book</dc:title><dc:creator>ViewIt</dc:creator><dc:identifier id="id">viewit-test</dc:identifier></metadata><manifest><item id="chapter" href="chapter.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="chapter"/></spine></package>`,
  );
  zip.file(
    "OPS/chapter.xhtml",
    `<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml"><head><title>Long chapter</title></head><body><h1>Readable chapter</h1><p><a href="#ending">Jump to the ending</a></p>${Array.from({ length: 45 }, (_, index) => `<p>Paragraph ${index + 1}: ViewIt keeps this chapter scrollable, selectable, and available to normal browser interaction.</p>`).join("")}<h2 id="ending">The ending</h2><p>End of interaction fixture.</p></body></html>`,
  );
  return zip.generateAsync({ type: "nodebuffer", compression: "DEFLATE" });
}

async function openEpub(page: Page): Promise<void> {
  await page.addInitScript(() => localStorage.setItem("viewit-onboarded", "1"));
  await page.goto("/");
  const chooserPromise = page.waitForEvent("filechooser");
  await page.getByRole("button", { name: "Open file" }).click();
  const chooser = await chooserPromise;
  await chooser.setFiles({
    name: "interaction.epub",
    mimeType: "application/epub+zip",
    buffer: await epubFixture(),
  });
  await expect(page.locator(".epub-fullscreen")).toBeVisible();
  await expect(page.locator(".epub-frame")).toBeVisible();
}

for (const viewport of [
  { name: "phone", width: 360, height: 780 },
  { name: "desktop", width: 1440, height: 900 },
] as const) {
  test(`EPUB remains scrollable and interactive at ${viewport.name}`, async ({ page }) => {
    await page.setViewportSize(viewport);
    await openEpub(page);

    const reader = await page.evaluate(() => {
      const frame = document.querySelector<HTMLIFrameElement>(".epub-frame");
      const body = frame?.contentDocument?.body;
      const html = frame?.contentDocument?.documentElement;
      const toolbarButtons = [...document.querySelectorAll<HTMLElement>(".reader-toolbar button")];
      const range = frame?.contentDocument?.createRange();
      const firstParagraph = body?.querySelector("p");
      if (range && firstParagraph) range.selectNodeContents(firstParagraph);
      return {
        touchLayers: document.querySelectorAll(".touch-layer").length,
        overflowY: html ? getComputedStyle(html).overflowY : "missing",
        scrollHeight: Math.max(html?.scrollHeight ?? 0, body?.scrollHeight ?? 0),
        clientHeight: frame?.clientHeight ?? 0,
        bodyTextLength: body?.innerText.length ?? 0,
        bodyChildren: body?.children.length ?? 0,
        lastChildBottom: body?.lastElementChild?.getBoundingClientRect().bottom ?? 0,
        hasLink: Boolean(body?.querySelector("a[href='#ending']")),
        selectableText: range?.toString().length ?? 0,
        targetHeights: toolbarButtons.map((button) => button.offsetHeight),
        horizontalOverflow:
          document.documentElement.scrollWidth > document.documentElement.clientWidth,
      };
    });

    expect(reader.touchLayers).toBe(0);
    expect(reader.overflowY).toBe("auto");
    expect(reader.bodyTextLength).toBeGreaterThan(2_000);
    expect(reader.bodyChildren).toBeGreaterThan(40);
    expect(reader.lastChildBottom).toBeGreaterThan(reader.clientHeight);
    expect(reader.scrollHeight).toBeGreaterThan(reader.clientHeight);
    expect(reader.hasLink).toBe(true);
    expect(reader.selectableText).toBeGreaterThan(0);
    expect(reader.horizontalOverflow).toBe(false);
    for (const height of reader.targetHeights) expect(height).toBeGreaterThanOrEqual(44);

    await page.locator(".epub-frame").hover();
    await page.mouse.wheel(0, 500);
    await expect
      .poll(() =>
        page.evaluate(() => {
          const frame = document.querySelector<HTMLIFrameElement>(".epub-frame");
          return Math.max(
            frame?.contentWindow?.scrollY ?? 0,
            frame?.contentDocument?.documentElement.scrollTop ?? 0,
            frame?.contentDocument?.body.scrollTop ?? 0,
          );
        }),
      )
      .toBeGreaterThan(0);

    const frame = page.frameLocator(".epub-frame");
    await frame.getByRole("link", { name: "Jump to the ending" }).click();
    await expect
      .poll(() =>
        page.evaluate(() => {
          const frame = document.querySelector<HTMLIFrameElement>(".epub-frame");
          return {
            hash: frame?.contentWindow?.location.hash ?? "",
            scrollY: frame?.contentWindow?.scrollY ?? 0,
          };
        }),
      )
      .toMatchObject({ hash: "#ending", scrollY: expect.any(Number) });
    await page.evaluate(() => {
      document.querySelector<HTMLIFrameElement>(".epub-frame")?.contentWindow?.scrollTo(0, 0);
    });

    await expect(page).toHaveScreenshot(`epub-reader-${viewport.name}.png`, {
      animations: "disabled",
    });
  });
}
