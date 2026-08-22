import { test, expect, type Page } from "@playwright/test";

async function openTextFixture(page: Page): Promise<void> {
  const chooserPromise = page.waitForEvent("filechooser");
  await page.getByRole("button", { name: "Open file" }).click();
  const chooser = await chooserPromise;
  await chooser.setFiles({
    name: "back.txt",
    mimeType: "text/plain",
    buffer: Buffer.from("back navigation probe\n".repeat(50), "utf-8"),
  });
}

test("Alt+Left returns from a document to the browse screen", async ({ page }) => {
  await page.goto("/");
  await openTextFixture(page);
  await expect(page.locator(".text-viewer")).toBeVisible();

  // The header fs-toggle is enabled once a doc is open; the mode button says
  // "Return to viewer" only while in browse mode — after back we must be back
  // in browse with the viewer gone.
  await page.keyboard.press("Alt+ArrowLeft");
  await expect(page.locator(".text-viewer")).toHaveCount(0);
  await expect(page.locator(".mode-toggle")).toHaveAttribute("aria-label", "Return to viewer");
});

test("back closes overlays before leaving a document", async ({ page }) => {
  await page.goto("/");
  await openTextFixture(page);
  await expect(page.locator(".text-viewer")).toBeVisible();

  // Open the debug panel overlay.
  await page.getByRole("button", { name: "Show debug log" }).click();
  await expect(page.locator(".debug-panel")).toBeVisible();

  // First back press closes ONLY the overlay.
  await page.keyboard.press("Alt+ArrowLeft");
  await expect(page.locator(".debug-panel")).toBeHidden();
  await expect(page.locator(".text-viewer")).toBeVisible();

  // Second press leaves the document.
  await page.keyboard.press("Alt+ArrowLeft");
  await expect(page.locator(".text-viewer")).toHaveCount(0);
});

test("back exits full screen first, then leaves the document", async ({ page }) => {
  await page.goto("/");
  await openTextFixture(page);
  await expect(page.locator(".text-viewer")).toBeVisible();

  await page.locator(".app-header .fs-toggle").click();
  await expect(page.locator(".viewit-root")).toHaveClass(/fs-mode/);

  await page.keyboard.press("Alt+ArrowLeft");
  await expect(page.locator(".viewit-root")).not.toHaveClass(/fs-mode/);
  await expect(page.locator(".text-viewer")).toBeVisible();

  await page.keyboard.press("Alt+ArrowLeft");
  await expect(page.locator(".text-viewer")).toHaveCount(0);
});
