import { defineConfig, devices } from "@playwright/test";

/**
 * Perf harness:
 * - `browser` — Vite demo in Chromium (plain Playwright; CI-friendly)
 * - `tauri` — real Lattice WebView via `tauri-plugin-playwright`
 *   (start with `pnpm test:perf:tauri` or `pnpm tauri:dev:e2e` + `--project=tauri`)
 */
export default defineConfig({
  testDir: "./e2e",
  testMatch: "**/perf/**/*.spec.ts",
  fullyParallel: false,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  workers: 1,
  timeout: 60_000,
  reporter: [["list"], ["html", { open: "never", outputFolder: "playwright-report" }]],
  outputDir: "test-results",
  use: {
    baseURL: "http://localhost:5173",
    trace: "on-first-retry",
  },
  projects: [
    {
      name: "browser",
      testIgnore: "**/*.tauri.perf.spec.ts",
      use: { ...devices["Desktop Chrome"] },
    },
    {
      name: "tauri",
      testMatch: "**/*.tauri.perf.spec.ts",
      use: {
        // @ts-expect-error custom fixture option from @srsholmes/tauri-playwright
        mode: "tauri",
        // Playwright browser traces capture a blank page in Tauri mode.
        trace: "off",
        screenshot: "off",
      },
    },
  ],
  webServer: process.env.LATTICE_PERF_SKIP_WEBSERVER
    ? undefined
    : {
        command: "pnpm --filter @lattice/desktop dev",
        url: "http://localhost:5173",
        reuseExistingServer: !process.env.CI,
        timeout: 120_000,
        cwd: "../..",
      },
});
