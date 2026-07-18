import { defineConfig, devices } from "@playwright/test";

/**
 * Browser-mode perf harness for the Vite desktop demo (`inBrowser` fixture).
 * Measures shell and page-open budgets from docs/23-frontend-rendering-and-performance.md.
 */
export default defineConfig({
  testDir: "./e2e",
  testMatch: "**/perf/**/*.spec.ts",
  fullyParallel: false,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  workers: 1,
  reporter: [["list"], ["html", { open: "never", outputFolder: "playwright-report" }]],
  outputDir: "test-results",
  use: {
    baseURL: "http://localhost:5173",
    trace: "on-first-retry",
  },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],
  webServer: {
    command: "pnpm --filter @lattice/desktop dev",
    url: "http://localhost:5173",
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
    cwd: "../..",
  },
});
