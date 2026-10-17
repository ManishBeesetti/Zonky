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
    await page.addInitScript(() => {
      window.__TAURI_MOCK_OVERRIDES__ = {
        list_loaded_models: [],
      };
    });
    await page.goto("/");
    await page.locator("nav button", { hasText: "Chat" }).click();

    await expect(
      page.locator("main > div:not([hidden])").getByText("No models loaded")
    ).toBeVisible();
  });

  test("should forward inference parameters and system prompt on send", async ({
    page,
  }) => {
    await page.goto("/");
    await page.locator("nav button", { hasText: "Chat" }).click();

    // Select Coder system prompt preset
    await page.locator("main aside button", { hasText: "Coder" }).click();

    const textarea = page.locator(
      'main textarea[placeholder="Enter inference instructions..."]'
    );
    await textarea.fill("Write a function");
    await page.locator("main button", { hasText: "Run Inference" }).click();

    await expect(
      page.locator("main").getByText(
        "Hello! I'm a test response from the mocked inference backend."
      )
    ).toBeVisible({ timeout: 5000 });

    const invokeLog = await page.evaluate(() => window.__TAURI_INVOKE_LOG__);
    const chatCall = invokeLog.find((entry) => entry.cmd === "chat_complete");
    expect(chatCall).toBeTruthy();
    expect(chatCall.args.temperature).toBe(0.75);
    expect(chatCall.args.topP).toBe(0.9);
    expect(chatCall.args.repeatPenalty).toBe(1.1);
    expect(chatCall.args.maxTokens).toBe(2048);
    expect(chatCall.args.messages[0].role).toBe("system");
  });

  test("should allow quick-loading downloaded models directly from Chat", async ({
    page,
  }) => {
    await page.goto("/");
    await page.locator("nav button", { hasText: "Chat" }).click();

    // test-model-2 is downloaded in MOCK_LOCAL_MODELS but not loaded in MOCK_LOADED_MODELS
    const quickLoadBtn = page
      .locator("main > div:not([hidden])")
      .locator("button", { hasText: "test-model-2" });
    await expect(quickLoadBtn).toBeVisible();
    await quickLoadBtn.click();

    await expect
      .poll(async () => {
        const invokeLog = await page.evaluate(() => window.__TAURI_INVOKE_LOG__);
        return invokeLog.some(
          (entry) => entry.cmd === "load_model" && entry.args.modelId === "test-model-2"
        );
      })
      .toBe(true);
  });
});

