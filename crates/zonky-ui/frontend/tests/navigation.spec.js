import { test, expect } from "./fixtures.js";

test.describe("Navigation", () => {
  test("should load the app and show Dashboard by default", async ({ page }) => {
    await page.goto("/");
    // App title in nav
    await expect(page.locator("nav").locator("text=ZONKY")).toBeVisible();
    // Dashboard should be the default page
    await expect(page.getByRole("heading", { name: "Compute Dashboard" })).toBeVisible();
  });

  test("should navigate to all pages via sidebar", async ({ page }) => {
    await page.goto("/");

    // Navigate to Library
    await page.locator("nav button", { hasText: "Library" }).click();
    await expect(page.getByRole("heading", { name: "Local Library" })).toBeVisible();

    // Navigate to Models (Hub)
    await page.locator("nav button", { hasText: "Models" }).click();
    await expect(page.getByRole("heading", { name: "Model Hub" })).toBeVisible();

    // Navigate to Downloads
    await page.locator("nav button", { hasText: "Downloads" }).click();
    await expect(page.getByRole("heading", { name: "Downloads", exact: true })).toBeVisible();

    // Navigate to Chat
    await page.locator("nav button", { hasText: "Chat" }).click();
    await expect(page.getByRole("heading", { name: "Start Inference" })).toBeVisible();

    // Navigate to Settings
    await page.locator("nav button", { hasText: "Settings" }).click();
    await expect(page.getByRole("heading", { name: "Settings" })).toBeVisible();

    // Navigate back to Dashboard
    await page.locator("nav button", { hasText: "Dashboard" }).click();
    await expect(page.getByRole("heading", { name: "Compute Dashboard" })).toBeVisible();
  });

  test("should show status bar with VRAM info", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator("footer")).toBeVisible();
    await expect(page.locator("footer").locator("text=System Ready")).toBeVisible();
    // VRAM should display from mock data
    await expect(page.locator("footer").locator("text=/VRAM/")).toBeVisible();
  });
});
