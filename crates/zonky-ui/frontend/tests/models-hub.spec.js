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

  test("shows an error state when model search fails", async ({ page }) => {
    await page.goto("/");
    await page.locator("nav button", { hasText: "Models" }).click();

    await page.evaluate(() => {
      const original = window.__TAURI_INTERNALS__.invoke;
      window.__TAURI_INTERNALS__.invoke = async (cmd, args, options) => {
        if (cmd === "search_models") {
          throw new Error("Hub API unavailable");
        }
        return original(cmd, args, options);
      };
    });

    await page.getByPlaceholder("Search models by name or task...").fill("llama");
    await page.getByRole("button", { name: "Search Hub" }).click();

    await expect(page.getByRole("heading", { name: "Search failed" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Retry search" })).toBeVisible();
  });

  test("shows a no-results state when search returns empty", async ({ page }) => {
    await page.goto("/");
    await page.locator("nav button", { hasText: "Models" }).click();

    await page.evaluate(() => {
      const original = window.__TAURI_INTERNALS__.invoke;
      window.__TAURI_INTERNALS__.invoke = async (cmd, args, options) => {
        if (cmd === "search_models") {
          return [];
        }
        return original(cmd, args, options);
      };
    });

    await page.getByPlaceholder("Search models by name or task...").fill("no-such-model");
    await page.getByRole("button", { name: "Search Hub" }).click();

    await expect(page.getByRole("heading", { name: "No models found" })).toBeVisible();
  });

  test("renders deterministic icon colors for model cards", async ({ page }) => {
    await page.goto("/");
    await page.locator("nav button", { hasText: "Models" }).click();

    await page.getByPlaceholder("Search models by name or task...").fill("llama");
    await page.getByRole("button", { name: "Search Hub" }).click();

    const llamaCard = page
      .locator("div", { hasText: "Llama-3.2-3B-Instruct-GGUF" })
      .first();
    const iconWrapper = llamaCard.locator("div.w-10.h-10").first();

    await expect(iconWrapper).toHaveClass(/text-primary/);
    await expect(iconWrapper).toHaveClass(/bg-primary\/10/);
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
