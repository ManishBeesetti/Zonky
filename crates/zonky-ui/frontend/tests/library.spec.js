import { test, expect } from "./fixtures.js";

// Helper: scope to visible page
function visiblePage(page) {
  return page.locator("main > div:not([hidden])");
}

test.describe("Library", () => {
  test("should display downloaded models", async ({ page }) => {
    await page.goto("/");
    await page.locator("nav button", { hasText: "Library" }).click();

    await expect(page.getByRole("heading", { name: "Local Library" })).toBeVisible();
    await expect(visiblePage(page).getByText("Downloaded Models")).toBeVisible();

    // Mock models should appear
    await expect(
      visiblePage(page).getByText("TestOrg/SmallModel-7B-GGUF")
    ).toBeVisible();
    await expect(
      visiblePage(page).getByText("TestOrg/TinyModel-1B-GGUF")
    ).toBeVisible();
  });

  test("should show backend compatibility badges", async ({ page }) => {
    await page.goto("/");
    await page.locator("nav button", { hasText: "Library" }).click();

    // Backend badges should be visible for each model (candle, llamacpp, rocm, cuda)
    await expect(page.locator("main").getByText("candle").first()).toBeVisible();
  });

  test("should show VRAM requirements", async ({ page }) => {
    await page.goto("/");
    await page.locator("nav button", { hasText: "Library" }).click();

    // Model size should be displayed
    await expect(page.locator("main").getByText("4.0 GB")).toBeVisible();
    await expect(page.locator("main").getByText("1.0 GB")).toBeVisible();
  });

  test("should have play and delete buttons for each model", async ({
    page,
  }) => {
    await page.goto("/");
    await page.locator("nav button", { hasText: "Library" }).click();

    // Play buttons (load model) - scoped to visible page
    const playButtons = visiblePage(page).locator("span.material-symbols-outlined", {
      hasText: "play_arrow",
    });
    await expect(playButtons).toHaveCount(2);

    // Delete buttons - scoped to visible page
    const deleteButtons = visiblePage(page).locator("span.material-symbols-outlined", {
      hasText: "delete",
    });
    await expect(deleteButtons).toHaveCount(2);
  });

  test("should show loading state when play button is clicked", async ({
    page,
  }) => {
    await page.goto("/");
    await page.locator("nav button", { hasText: "Library" }).click();

    // Click the first play button
    const playButton = page.locator("main")
      .locator("button", {
        has: page.locator("span", { hasText: "play_arrow" }),
      })
      .first();
    await playButton.click();

    // Should show loading spinner (progress_activity)
    await expect(
      page.locator("main span.material-symbols-outlined", {
        hasText: "progress_activity",
      })
    ).toBeVisible({ timeout: 2000 });
  });

  test("should show empty state when no models downloaded", async ({
    page,
  }) => {
    // This test uses custom overrides via the fixture
    test.skip(); // Skip for now — overrides need fixture-level setup
  });
});
