import { test, expect } from "@playwright/test";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import {
  makeStressText,
  STRESS_NEEDLE,
  STRESS_FILE_NAME,
} from "../../../scripts/stress-fixture.mjs";

// A real on-disk file (not an in-memory buffer): Chromium splices buffer-backed
// intercepted files synchronously inside the renderer, which would poison the
// freeze-budget measurement with test-harness cost that real users never pay.
const stressPath = (() => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "viewit-stress-"));
  const file = path.join(dir, STRESS_FILE_NAME);
  fs.writeFileSync(file, makeStressText({ megabytes: 4 }));
  return file;
})();

async function openStressFixture(page: import("@playwright/test").Page): Promise<void> {
  const chooserPromise = page.waitForEvent("filechooser");
  await page.getByRole("button", { name: "Open file" }).click();
  const chooser = await chooserPromise;
  await chooser.setFiles(stressPath);
}

interface ChromeGeometry {
  header: { x: number; y: number; width: number; height: number };
  buttons: { x: number; y: number; width: number; height: number }[];
}

async function readChromeGeometry(page: import("@playwright/test").Page): Promise<ChromeGeometry> {
  return page.evaluate(() => {
    const rect = (el: Element) => {
      const r = el.getBoundingClientRect();
      return {
        x: Math.round(r.x),
        y: Math.round(r.y),
        width: Math.round(r.width),
        height: Math.round(r.height),
      };
    };
    const header = document.querySelector(".app-header");
    if (!header) throw new Error("missing .app-header");
    return {
      header: rect(header),
      buttons: Array.from(header.querySelectorAll("button")).map(rect),
    };
  });
}

// The fullscreen toggle appears contextually once a document is open (by
// design). The freeze contract covers the pre-existing controls: every button
// present before the open must keep its exact rect afterwards.
function assertFrozenControls(before: ChromeGeometry, after: ChromeGeometry): void {
  expect(after.header).toEqual(before.header);
  expect(after.buttons.slice(0, before.buttons.length)).toEqual(before.buttons);
}

test("header geometry stays frozen while a multi-MB text file loads", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".app-header")).toBeVisible();

  // Long-task watchdog: the frozen-chrome contract is that mounting a huge
  // document never blocks the main thread for a perceptible interval.
  await page.evaluate(() => {
    (window as any).__longTasks = [];
    new PerformanceObserver((list) => {
      for (const entry of list.getEntries()) {
        (window as any).__longTasks.push(entry.duration);
      }
    }).observe({ entryTypes: ["longtask"] });
  });

  const before = await readChromeGeometry(page);

  await openStressFixture(page);

  // The open pipeline can complete in well under a second, so do not gate on
  // the transient busy card — just verify the chrome did not move at the
  // earliest observable moment and stays put after mount.
  const during = await readChromeGeometry(page);
  assertFrozenControls(before, during);

  // Controls stay interactive while the document pipeline runs. The debug-log
  // toggle is a cheap local overlay (unlike theme toggling, which legitimately
  // restyles the whole document in one pass and is asserted separately below).
  await page.getByRole("button", { name: "Show debug log" }).click();
  await expect(page.locator(".debug-panel")).toBeVisible();
  await page.getByRole("button", { name: "Hide debug log" }).click();
  await expect(page.locator(".debug-panel")).toHaveCount(0);

  await expect(page.locator(".text-viewer .meta")).toContainText("bytes", {
    timeout: 30_000,
  });
  const after = await readChromeGeometry(page);
  assertFrozenControls(before, after);

  // Freeze budget for the pure open→mount path. The pre-fix regression
  // measured ~3.1s; a clean run stays well under 300ms of long tasks.
  const longTasks = await page.evaluate(() => (window as any).__longTasks as number[]);
  expect(longTasks.filter((d) => d > 300)).toEqual([]);

  // Post-mount interactivity: theme toggling restyles the whole document by
  // design, so it is exercised after the freeze-budget window.
  await page.getByRole("button", { name: "Toggle dark mode" }).click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await page.getByRole("button", { name: "Toggle dark mode" }).click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");

  // The window itself must never scroll — main is the sole scroll container.
  await page.locator("main").evaluate((el) => el.scrollTo(0, el.scrollHeight));
  await page.waitForFunction(() => window.scrollY === 0);
  const scrolled = await readChromeGeometry(page);
  expect(scrolled.header).toEqual(before.header);
});

test("wide stress lines never widen the page or shrink controls at phone width", async ({
  page,
}) => {
  await page.setViewportSize({ width: 360, height: 740 });
  await page.goto("/");
  await expect(page.locator(".app-header")).toBeVisible();

  const before = await readChromeGeometry(page);
  await openStressFixture(page);
  await expect(page.locator(".text-viewer .meta")).toContainText("bytes", {
    timeout: 30_000,
  });

  const overflow = await page.evaluate(() => ({
    docW: document.documentElement.scrollWidth,
    docC: document.documentElement.clientWidth,
    bodyW: document.body.scrollWidth,
    bodyC: document.body.clientWidth,
  }));
  expect(overflow.docW).toBeLessThanOrEqual(overflow.docC);
  expect(overflow.bodyW).toBeLessThanOrEqual(overflow.bodyC);

  const after = await readChromeGeometry(page);
  assertFrozenControls(before, after);
});

test("search still finds the needle in progressively mounted chunks", async ({ page }) => {
  await page.goto("/");
  await openStressFixture(page);
  await expect(page.locator(".text-viewer .meta")).toContainText("bytes", {
    timeout: 30_000,
  });

  await page.getByPlaceholder(/search/i).fill(STRESS_NEEDLE);
  await page.waitForFunction(
    () => document.querySelectorAll(".text-viewer mark").length > 0,
    undefined,
    { timeout: 15_000 },
  );
});
