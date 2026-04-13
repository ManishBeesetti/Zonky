import { test, expect } from "./fixtures.js";

test.describe("Models Hub", () => {
  test("should display the Model Hub page", async ({ page }) => {
    await page.goto("/");
    await page.locator("nav button", { hasText: "Models" }).click();

    await expect(page.getByRole("heading", { name: "Model Hub" })).toBeVisible();
  });

  test("should have a search input", async ({ page }) => {
    await page.goto("/");
    await page.locator("nav button", { hasText: "Models" }).click();

    const searchInput = page.locator(
      'input[placeholder*="Search"], input[placeholder*="search"]'
    );
    await expect(searchInput).toBeVisible();
  });
});

test.describe("Downloads", () => {
  test("should display the Downloads page", async ({ page }) => {
    await page.goto("/");
    await page.locator("nav button", { hasText: "Downloads" }).click();

    // Downloads page content
    await expect(page.getByRole("heading", { name: "Downloads", exact: true })).toBeVisible();
  });
});
