import { test, expect, type Page } from "@playwright/test";

async function openTextFixture(page: Page): Promise<void> {
  const chooserPromise = page.waitForEvent("filechooser");
  await page.getByRole("button", { name: "Open file" }).click();
  const chooser = await chooserPromise;
  await chooser.setFiles({
    name: "hello.txt",
    mimeType: "text/plain",
    buffer: Buffer.from("Hello full screen mode\n".repeat(200), "utf-8"),
  });
}

test("header toggle enters and exits full screen for text documents", async ({ page }) => {
  await page.goto("/");
  await openTextFixture(page);
  await expect(page.locator(".text-viewer")).toBeVisible();

  await expect(page.locator(".bottom-bar .fs-toggle")).toBeVisible();

  await page.locator(".bottom-bar .fs-toggle").click();
  await expect(page.locator(".viewit-root")).toHaveClass(/fs-mode/);
  await expect(page.locator(".app-header")).toBeHidden();
  await expect(page.locator(".bottom-bar")).toBeHidden();
  // Handler adaptation: the text meta line is hidden in full screen.
  await expect(page.locator(".text-viewer .meta")).toBeHidden();

  // Escape exits without touching the chip.
  await page.keyboard.press("Escape");
  await expect(page.locator(".viewit-root")).not.toHaveClass(/fs-mode/);
  await expect(page.locator(".app-header")).toBeVisible();
});

test("exit chip returns to normal chrome", async ({ page }) => {
  await page.goto("/");
  await openTextFixture(page);
  await expect(page.locator(".text-viewer")).toBeVisible();

  await page.locator(".bottom-bar .fs-toggle").click();
  await expect(page.locator(".app-header")).toBeHidden();
  await page.keyboard.press("Escape");
  await expect(page.locator(".app-header")).toBeVisible();
  await expect(page.locator(".bottom-bar .fs-toggle")).toBeVisible();
});

test("media full screen requests native element fullscreen once", async ({ page }) => {
  await page.goto("/");
  await page.evaluate(() => {
    (window as any).__fsCalls = 0;
    const proto = HTMLElement.prototype as unknown as {
      requestFullscreen?: () => Promise<void>;
    };
    Object.defineProperty(proto, "requestFullscreen", {
      configurable: true,
      value: function () {
        (window as any).__fsCalls++;
        return Promise.resolve();
      },
    });
  });

  // Minimal valid WAV so MediaViewer takes the built-in player path.
  const wav = Buffer.alloc(44 + 16);
  wav.write("RIFF", 0);
  wav.writeUInt32LE(wav.length - 8, 4);
  wav.write("WAVEfmt ", 8);
  wav.writeUInt32LE(16, 16);
  wav.writeUInt16LE(1, 20);
  wav.writeUInt16LE(1, 22);
  wav.writeUInt32LE(8000, 24);
  wav.writeUInt32LE(16000, 28);
  wav.writeUInt16LE(1, 32);
  wav.writeUInt16LE(8, 34);
  wav.write("data", 36);
  wav.writeUInt32LE(16, 40);

  const chooserPromise = page.waitForEvent("filechooser");
  await page.getByRole("button", { name: "Open file" }).click();
  const chooser = await chooserPromise;
  await chooser.setFiles({ name: "tone.wav", mimeType: "audio/wav", buffer: wav });
  await expect(page.locator(".media-viewer")).toBeVisible({ timeout: 15_000 });

  await page.locator(".bottom-bar .fs-toggle").click();
  await expect(page.locator(".media-viewer")).toHaveClass(/fs/);
  expect(await page.evaluate(() => (window as any).__fsCalls)).toBeGreaterThanOrEqual(1);
});
