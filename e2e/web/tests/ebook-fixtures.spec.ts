import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { expect, test, type Page } from "@playwright/test";

const fixtures = [
  { name: "sample.epub", mimeType: "application/epub+zip" },
  { name: "sample.mobi", mimeType: "application/x-mobipocket-ebook" },
] as const;

async function openFixture(page: Page, fixture: (typeof fixtures)[number]): Promise<void> {
  await page.addInitScript(() => localStorage.setItem("viewit-onboarded", "1"));
  await page.goto("/");
  const chooserPromise = page.waitForEvent("filechooser");
  await page.getByRole("button", { name: "Open file" }).click();
  const chooser = await chooserPromise;
  await chooser.setFiles({
    name: fixture.name,
    mimeType: fixture.mimeType,
    buffer: await readFile(resolve("build/testing_viewit", fixture.name)),
  });
  await expect(page.locator(".epub-fullscreen")).toBeVisible();
  await expect(page.locator(".epub-frame")).toBeVisible();
}

for (const fixture of fixtures) {
  test(`${fixture.name} renders real readable content`, async ({ page }) => {
    await page.setViewportSize({ width: 360, height: 780 });
    await openFixture(page, fixture);

    const metrics = await page.evaluate(() => {
      const frame = document.querySelector<HTMLIFrameElement>(".epub-frame");
      const body = frame?.contentDocument?.body;
      return {
        textLength: body?.innerText.trim().length ?? 0,
        images: body?.querySelectorAll("img").length ?? 0,
        decodedImages: [...(body?.querySelectorAll("img") ?? [])].filter(
          (image) => image.complete && image.naturalWidth > 0,
        ).length,
        svgImages: body?.querySelectorAll("svg image").length ?? 0,
        inlinedSvgImages: [...(body?.querySelectorAll("svg image") ?? [])].filter((image) => {
          const href = image.getAttribute("href") ?? image.getAttribute("xlink:href") ?? "";
          return href.startsWith("data:image/");
        }).length,
        stylesheetLinks: body?.ownerDocument.querySelectorAll('link[rel="stylesheet"]').length ?? 0,
        touchLayers: document.querySelectorAll(".touch-layer").length,
        targetHeights: [...document.querySelectorAll<HTMLElement>(".reader-toolbar button")].map(
          (button) => button.offsetHeight,
        ),
        horizontalOverflow:
          document.documentElement.scrollWidth > document.documentElement.clientWidth,
      };
    });

    expect(
      metrics.textLength > 40 || metrics.decodedImages > 0 || metrics.inlinedSvgImages > 0,
    ).toBe(true);
    expect(metrics.decodedImages).toBe(metrics.images);
    expect(metrics.inlinedSvgImages).toBe(metrics.svgImages);
    expect(metrics.stylesheetLinks).toBe(0);
    expect(metrics.touchLayers).toBe(0);
    expect(metrics.horizontalOverflow).toBe(false);
    for (const height of metrics.targetHeights) expect(height).toBeGreaterThanOrEqual(44);

    await expect(page).toHaveScreenshot(`ebook-${fixture.name}.png`, {
      animations: "disabled",
    });
  });
}
