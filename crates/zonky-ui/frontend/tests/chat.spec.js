import { test, expect } from "./fixtures.js";

test.describe("Chat", () => {
  test("should show model picker with loaded models", async ({ page }) => {
    await page.goto("/");
    await page.locator("nav button", { hasText: "Chat" }).click();

    // Model picker should be visible
    await expect(page.locator("main select")).toBeVisible();

    // Should have the loaded model as an option
    await expect(page.locator("main select option")).toHaveCount(1);
    await expect(page.locator("main select")).toHaveValue("test-model-1");
  });

  test("should show empty state with Start Inference prompt", async ({
    page,
  }) => {
    await page.goto("/");
    await page.locator("nav button", { hasText: "Chat" }).click();

    await expect(page.getByRole("heading", { name: "Start Inference" })).toBeVisible();
  });

  test("should show refresh button for model list", async ({ page }) => {
    await page.goto("/");
    await page.locator("nav button", { hasText: "Chat" }).click();

    await expect(
      page.locator("main button", { hasText: "Refresh" })
    ).toBeVisible();
  });

  test("should have chat input with placeholder", async ({ page }) => {
    await page.goto("/");
    await page.locator("nav button", { hasText: "Chat" }).click();

    const textarea = page.locator(
      'main textarea[placeholder="Enter inference instructions..."]'
    );
    await expect(textarea).toBeVisible();
  });

  test("should have Run Inference button", async ({ page }) => {
    await page.goto("/");
    await page.locator("nav button", { hasText: "Chat" }).click();

    await expect(
      page.locator("main button", { hasText: "Run Inference" })
    ).toBeVisible();
  });

  test("should disable Run Inference when input is empty", async ({
    page,
  }) => {
    await page.goto("/");
    await page.locator("nav button", { hasText: "Chat" }).click();

    const runButton = page.locator("main button", { hasText: "Run Inference" });
    await expect(runButton).toBeDisabled();
  });

  test("should send message and display response", async ({ page }) => {
    await page.goto("/");
    await page.locator("nav button", { hasText: "Chat" }).click();

    // Type a message
    const textarea = page.locator(
      'main textarea[placeholder="Enter inference instructions..."]'
    );
    await textarea.fill("Hello, test message!");

    // Click Run Inference
    await page.locator("main button", { hasText: "Run Inference" }).click();

    // User message should appear
    await expect(page.locator("main").getByText("Hello, test message!")).toBeVisible();

    // Assistant response from mock should appear
    await expect(
      page.locator("main").getByText(
        "Hello! I'm a test response from the mocked inference backend."
      )
    ).toBeVisible({ timeout: 5000 });
  });

  test("should clear chat when Clear Chat button is clicked", async ({
    page,
  }) => {
    await page.goto("/");
    await page.locator("nav button", { hasText: "Chat" }).click();

    // Send a message first
    const textarea = page.locator(
      'main textarea[placeholder="Enter inference instructions..."]'
    );
    await textarea.fill("Test message");
    await page.locator("main button", { hasText: "Run Inference" }).click();

    // Wait for response
    await expect(
      page.locator("main").getByText(
        "Hello! I'm a test response from the mocked inference backend."
      )
    ).toBeVisible({ timeout: 5000 });

    // Click Clear Chat
    await page.locator("main button", { hasText: "Clear Chat" }).click();

    // Messages should be gone, empty state should return
    await expect(page.getByRole("heading", { name: "Start Inference" })).toBeVisible();
  });

  test("should show inference config panel", async ({ page }) => {
    await page.goto("/");
    await page.locator("nav button", { hasText: "Chat" }).click();

    await expect(page.locator("main").getByText("Inference Config")).toBeVisible();
    await expect(page.locator("main").getByText("Temperature")).toBeVisible();
    await expect(page.locator("main").getByText("Top-P")).toBeVisible();
    await expect(page.locator("main").getByText("Repeat Penalty")).toBeVisible();
  });

  test("should show hardware monitor in sidebar", async ({ page }) => {
    await page.goto("/");
    await page.locator("nav button", { hasText: "Chat" }).click();

    // Scope to the chat aside panel to avoid matching Dashboard
    const chatAside = page.locator("main aside");
    await expect(chatAside.getByText("Hardware Monitor")).toBeVisible();
    await expect(chatAside.getByText("GPU Memory")).toBeVisible();
    await expect(chatAside.getByText("Neural Engine")).toBeVisible();
  });

  test("should show no models loaded hint when none loaded", async ({
    page,
  }) => {
    test.skip();
  });
});
