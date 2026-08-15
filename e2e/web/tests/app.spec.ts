import { test, expect } from "@playwright/test";
import JSZip from "jszip";

async function iworkFixture(path: string, content: string): Promise<Buffer> {
  const zip = new JSZip();
  zip.file(path, content);
  return zip.generateAsync({ type: "nodebuffer", compression: "DEFLATE" });
}

async function openFixture(
  page: import("@playwright/test").Page,
  name: string,
  buffer: Buffer,
): Promise<void> {
  const chooserPromise = page.waitForEvent("filechooser");
  await page.getByRole("button", { name: "Open file" }).click();
  const chooser = await chooserPromise;
  await chooser.setFiles({ name, mimeType: "application/zip", buffer });
}

test("loads the app shell with an open-file affordance", async ({ page }) => {
  await page.goto("/");

  await expect(page).toHaveTitle(/ViewIt/);
  await expect(page.getByRole("heading", { name: "ViewIt" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Open file" })).toBeVisible();
});

test("top-level navigation is reachable for the viewer shell", async ({ page }) => {
  await page.goto("/");

  await expect(page.locator(".viewit-root")).toBeVisible();
  await expect(page.getByRole("heading", { level: 1 })).toContainText("ViewIt");
});

test("Pages uses the structured iWork preview instead of the OOXML reader", async ({ page }) => {
  await page.goto("/");
  await openFixture(page, "sample.pages", await iworkFixture("Data/document.txt", "Hello Pages\n"));

  await expect(page.getByText("Apple Pages", { exact: true })).toBeVisible();
  await expect(page.getByText("Best-effort partial preview from the iWork package.")).toBeVisible();
  await expect(page.getByText("Hello Pages", { exact: true })).toBeVisible();
  await expect(page.getByText(/Open a \.docx/)).toHaveCount(0);
});

test("Numbers and Keynote use their structured viewers", async ({ page }) => {
  await page.goto("/");
  await openFixture(
    page,
    "sample.numbers",
    await iworkFixture("Data/table.csv", "Quarter,Revenue\nQ1,42\n"),
  );

  await expect(page.getByText("Apple Numbers", { exact: true })).toBeVisible();
  await expect(page.getByText("Revenue", { exact: true })).toBeVisible();
  await expect(page.getByText("42", { exact: true })).toBeVisible();

  await openFixture(page, "sample.key", await iworkFixture("Data/slide.txt", "Hello Keynote\n"));

  await expect(page.getByText("Apple Keynote", { exact: true })).toBeVisible();
  await expect(page.getByText(/Hello Keynote/)).toBeVisible();
  await expect(page.getByText(/iWork package text extraction/)).toBeVisible();
});
