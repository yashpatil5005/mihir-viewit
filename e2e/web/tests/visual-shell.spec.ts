import { expect, test } from "@playwright/test";

const viewports = [
  { name: "phone", width: 320, height: 720 },
  { name: "desktop", width: 1440, height: 900 },
  { name: "ultrawide", width: 1920, height: 1080 },
] as const;

for (const viewport of viewports) {
  test(`empty shell remains stable at ${viewport.name}`, async ({ page }) => {
    await page.setViewportSize(viewport);
    await page.addInitScript(() => localStorage.setItem("viewit-onboarded", "1"));
    await page.goto("/");
    await expect(page.getByRole("heading", { name: "ViewIt" })).toBeVisible();

    const geometry = await page.evaluate(() => ({
      viewportWidth: document.documentElement.clientWidth,
      scrollWidth: document.documentElement.scrollWidth,
      headerTargets: [...document.querySelectorAll<HTMLElement>(".app-header button")].map(
        (element) => ({ width: element.offsetWidth, height: element.offsetHeight }),
      ),
    }));

    expect(geometry.scrollWidth).toBeLessThanOrEqual(geometry.viewportWidth);
    for (const target of geometry.headerTargets) {
      expect(target.width).toBeGreaterThanOrEqual(44);
      expect(target.height).toBeGreaterThanOrEqual(44);
    }

    await expect(page).toHaveScreenshot(`empty-shell-${viewport.name}.png`, {
      animations: "disabled",
      fullPage: true,
    });
  });
}
