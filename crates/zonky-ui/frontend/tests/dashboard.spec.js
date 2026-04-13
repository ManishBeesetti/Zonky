import { test, expect } from "./fixtures.js";

// Helper: scope to the visible dashboard page div
function dashboard(page) {
  return page.locator("main > div:not([hidden])");
}

test.describe("Dashboard", () => {
  test("should display GPU info from system", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByRole("heading", { name: "Compute Dashboard" }).first()).toBeVisible();

    // GPU name in hardware monitor
    await expect(dashboard(page).getByText("AMD Radeon Strix Halo").first()).toBeVisible();

    // Hardware monitor section
    await expect(dashboard(page).getByText("Hardware Monitor").first()).toBeVisible();
  });

  test("should display loaded models section", async ({ page }) => {
    await page.goto("/");
    // With a loaded model, should show model id and READY badge
    await expect(dashboard(page).getByText("test-model-1")).toBeVisible();
    await expect(dashboard(page).getByText("Ready")).toBeVisible();
  });

  test("should display inference stats", async ({ page }) => {
    await page.goto("/");
    await expect(dashboard(page).getByText("Throughput")).toBeVisible();
    await expect(dashboard(page).getByText("Latency")).toBeVisible();
    await expect(dashboard(page).getByText("Models in VRAM")).toBeVisible();
  });

  test("should display server status controls", async ({ page }) => {
    await page.goto("/");
    await expect(dashboard(page).getByText("Status:")).toBeVisible();
    await expect(dashboard(page).getByText("Stopped")).toBeVisible();
  });

  test("should display compute dependencies", async ({ page }) => {
    await page.goto("/");
    await expect(dashboard(page).getByText("Compute Dependencies")).toBeVisible();
  });

  test("should show system RAM info", async ({ page }) => {
    await page.goto("/");
    await expect(dashboard(page).getByText("System RAM")).toBeVisible();
  });
});
