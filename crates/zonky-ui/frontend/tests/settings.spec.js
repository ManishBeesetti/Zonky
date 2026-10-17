import { test, expect } from "./fixtures.js";

test.describe("Settings", () => {
  test("should edit and save configuration", async ({ page }) => {
    await page.goto("/");
    await page.locator("nav button", { hasText: "Settings" }).click();

    await expect(page.getByRole("heading", { name: "Settings" })).toBeVisible();

    const cacheInput = page.locator("#cache-dir-input");
    await expect(cacheInput).toHaveValue("/home/user/.cache/zonky");
    await cacheInput.fill("/tmp/zonky-custom-cache");

    const portInput = page.locator("#server-port-input");
    await portInput.fill("9090");

    await page.locator("main button", { hasText: "Save Configuration" }).click();

    await expect(
      page.locator("main").getByText("Settings saved successfully.")
    ).toBeVisible();

    const invokeLog = await page.evaluate(() => window.__TAURI_INVOKE_LOG__);
    const saveCall = invokeLog.find((entry) => entry.cmd === "save_config");
    expect(saveCall).toBeTruthy();
    expect(saveCall.args.config.cache_dir).toBe("/tmp/zonky-custom-cache");
    expect(saveCall.args.config.server.port).toBe(9090);
  });

  test("should clear model cache from settings", async ({ page }) => {
    await page.goto("/");
    await page.locator("nav button", { hasText: "Settings" }).click();

    await page.locator("main button", { hasText: "Clear Cache" }).click();

    await expect(
      page.locator("main").getByText("Cleared model cache (2 models removed).")
    ).toBeVisible();
  });
});
