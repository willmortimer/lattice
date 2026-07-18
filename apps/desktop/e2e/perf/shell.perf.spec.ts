import { expect, test } from "@playwright/test";
import { documentedTargets, perfBudgets, readNavigationMetrics } from "./budgets";
import { formatMs, readJsHeap, waitForShellChrome } from "./helpers";

test.describe("shell chrome", () => {
  test("cold start reaches visible workspace chrome within budget", async ({ page }) => {
    const startedAt = Date.now();
    await page.goto("/");
    await waitForShellChrome(page);
    const elapsedMs = Date.now() - startedAt;

    const nav = await readNavigationMetrics(page);
    const heap = await readJsHeap(page);

    test.info().annotations.push({
      type: "perf",
      description: [
        `wall=${formatMs(elapsedMs)}`,
        nav.domContentLoadedMs !== null ? `dcl=${formatMs(nav.domContentLoadedMs)}` : null,
        nav.loadEventEndMs !== null ? `load=${formatMs(nav.loadEventEndMs)}` : null,
        heap ? `heap=${(heap.usedJsHeapBytes / (1024 * 1024)).toFixed(2)} MiB` : null,
      ]
        .filter(Boolean)
        .join(" "),
    });

    expect(
      elapsedMs,
      `cold shell chrome should appear within ${perfBudgets.shellColdMs} ms (got ${elapsedMs} ms)`,
    ).toBeLessThanOrEqual(perfBudgets.shellColdMs);
  });

  test("warm reload reaches visible workspace chrome within budget", async ({ page }) => {
    await page.goto("/");
    await waitForShellChrome(page);

    const startedAt = Date.now();
    await page.reload();
    await waitForShellChrome(page);
    const elapsedMs = Date.now() - startedAt;

    test.info().annotations.push({
      type: "perf",
      description: `warm wall=${formatMs(elapsedMs)} (doc target ${documentedTargets.shellWarmMs} ms)`,
    });

    expect(
      elapsedMs,
      `warm shell chrome should appear within ${perfBudgets.shellWarmMs} ms (got ${elapsedMs} ms)`,
    ).toBeLessThanOrEqual(perfBudgets.shellWarmMs);
  });
});
