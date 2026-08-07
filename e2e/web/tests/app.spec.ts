import { test, expect } from "@playwright/test";

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
