import { createTauriTest } from "@srsholmes/tauri-playwright";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));

/**
 * Tauri-native Playwright fixtures via `tauri-plugin-playwright`.
 *
 * Browser mode from this package injects a mock `__TAURI_INTERNALS__`, which
 * flips Lattice out of the Vite demo fixture — so browser perf stays on plain
 * `@playwright/test`. These fixtures are for `--project=tauri` only.
 *
 * Expect the app to already be running with `--features e2e-testing`
 * (`pnpm tauri:dev:e2e` or `test:perf:tauri`).
 */
export const { test, expect } = createTauriTest({
  // Empty: tauri project only. Avoid per-test `location.href` reloads — the app
  // already loads Vite via Tauri `devUrl`, and reloads drop the plugin bridge.
  devUrl: "",
  mcpSocket: process.env.TAURI_PLAYWRIGHT_SOCKET ?? "/tmp/tauri-playwright.sock",
  tauriCwd: resolve(__dirname, ".."),
  startTimeout: 180,
});
