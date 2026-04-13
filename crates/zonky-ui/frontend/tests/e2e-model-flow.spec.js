import { test, expect } from "./fixtures.js";

test.describe("End-to-end Model Flow", () => {
  test("should search, download, load, and chat with a model", async ({ page }) => {
    // 1. Open the application
    await page.goto("/");
    await expect(page.getByRole("heading", { name: "Compute Dashboard" })).toBeVisible();

    // 2. Navigate to Models Hub and search for a model
    await page.locator("nav button", { hasText: "Models" }).click();
    await expect(page.getByRole("heading", { name: "Model Hub" })).toBeVisible();
    await page.getByPlaceholder("Search models by name or task...").fill("llama");
    await page.getByRole("button", { name: /Search Hub/i }).click();
    await expect(
      page.locator("main").getByText("Llama-3.2-3B-Instruct-GGUF")
    ).toBeVisible({ timeout: 5000 });

    // 3. Download a model — open file picker (scoped to main to avoid nav "Downloads")
    await page.locator("main button", { hasText: "Download" }).first().click();
    await expect(
      page.locator("main").getByText("Select a file to download")
    ).toBeVisible({ timeout: 5000 });

    // Select the Q4_K_M quantized file
    await page.locator("main button", { hasText: "q4_k_m" }).first().click();
    // Download is queued via the store; status bar shows confirmation
    await expect(
      page.locator("main").getByText(/Queued.*for download/i)
    ).toBeVisible({ timeout: 5000 });

    // 4. Navigate to Library and load a model
    await page.locator("nav button", { hasText: "Library" }).click();
    await expect(page.getByRole("heading", { name: "Local Library" })).toBeVisible();

    // 5. Click play on the first model and wait for it to load
    const playButton = page
      .locator("main")
      .locator("button")
      .filter({ has: page.locator("span", { hasText: "play_arrow" }) })
      .first();
    await playButton.click();
    // Mock load resolves instantly — spinner should briefly appear
    await expect(
      page.locator("main span", { hasText: "progress_activity" })
    ).toBeVisible({ timeout: 2000 });
    // Wait for load_model mock to resolve
    await page.waitForTimeout(500);

    // 6. Go to Chat and say hi
    await page.locator("nav button", { hasText: "Chat" }).click();
    await expect(page.getByRole("heading", { name: "Start Inference" })).toBeVisible();

    const textarea = page.locator(
      'main textarea[placeholder="Enter inference instructions..."]'
    );
    await textarea.fill("hi");
    await page.locator("main button", { hasText: "Run Inference" }).click();

    // User message should appear in the chat
    await expect(page.locator("main").getByText("hi", { exact: true })).toBeVisible();
    // Assistant mock response should appear
    await expect(
      page.locator("main").getByText(
        "Hello! I'm a test response from the mocked inference backend."
      )
    ).toBeVisible({ timeout: 5000 });
  });
});
