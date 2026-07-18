import { documentedTargets, perfBudgets, readNavigationMetrics } from "./budgets";
import { formatMs, waitForShellChrome } from "./helpers";
import { expect, test } from "../fixtures";

test.describe("shell chrome (tauri)", () => {
  test("shell reaches visible workspace chrome within budget after connect", async ({
    tauriPage,
  }) => {
    const startedAt = Date.now();
    await waitForShellChrome(tauriPage);
    const elapsedMs = Date.now() - startedAt;

    const nav = await readNavigationMetrics(tauriPage);
    test.info().annotations.push({
      type: "perf",
      description: [
        `tauri-ready wall=${formatMs(elapsedMs)}`,
        nav.domContentLoadedMs !== null ? `dcl=${formatMs(nav.domContentLoadedMs)}` : null,
        nav.loadEventEndMs !== null ? `load=${formatMs(nav.loadEventEndMs)}` : null,
      ]
        .filter(Boolean)
        .join(" "),
    });

    expect(
      elapsedMs,
      `tauri shell chrome should appear within ${perfBudgets.shellColdMs} ms (got ${elapsedMs} ms)`,
    ).toBeLessThanOrEqual(perfBudgets.shellColdMs);
  });

  test("warm reload reaches visible workspace chrome within budget", async ({ tauriPage }) => {
    await waitForShellChrome(tauriPage);

    const startedAt = Date.now();
    await tauriPage.reload();
    await waitForShellChrome(tauriPage);
    const elapsedMs = Date.now() - startedAt;

    test.info().annotations.push({
      type: "perf",
      description: `tauri warm wall=${formatMs(elapsedMs)} (doc target ${documentedTargets.shellWarmMs} ms)`,
    });

    expect(
      elapsedMs,
      `tauri warm shell chrome should appear within ${perfBudgets.shellWarmMs} ms (got ${elapsedMs} ms)`,
    ).toBeLessThanOrEqual(perfBudgets.shellWarmMs);
  });
});
