import { perfBudgets, readNavigationMetrics } from "./budgets";
import { formatMs, openTreePage, waitForShellChrome } from "./helpers";
import { expect, test } from "../fixtures";

const HOME_PAGE = "Page: Home.md";
const RESEARCH_PAGE = "Page: Research/Competitor Analysis.md";

test.describe("page open (tauri)", () => {
  test.beforeEach(async ({ tauriPage }) => {
    await waitForShellChrome(tauriPage);
  });

  test("opens a page from the resource tree within budget", async ({ tauriPage }) => {
    tauriPage.setDefaultTimeout(30_000);
    const startedAt = Date.now();
    await openTreePage(tauriPage, HOME_PAGE);
    await tauriPage.locator(".page-editor-content .ProseMirror").waitFor(30_000);
    await tauriPage.waitForFunction(
      `!!document.querySelector(".page-editor-content .ProseMirror")?.textContent?.includes("Kitchen-sink")`,
      30_000,
    );
    const elapsedMs = Date.now() - startedAt;

    const nav = await readNavigationMetrics(tauriPage);
    test.info().annotations.push({
      type: "perf",
      description: [
        `tauri page-open wall=${formatMs(elapsedMs)}`,
        `performance.now=${formatMs(nav.performanceNowMs)}`,
      ].join(" "),
    });

    expect(
      elapsedMs,
      `tauri page open should complete within ${perfBudgets.pageOpenMs} ms (got ${elapsedMs} ms)`,
    ).toBeLessThanOrEqual(perfBudgets.pageOpenMs);
  });

  test("scrolls open page content within budget", async ({ tauriPage }) => {
    tauriPage.setDefaultTimeout(30_000);
    await openTreePage(tauriPage, RESEARCH_PAGE);
    await tauriPage.locator(".page-editor-content .ProseMirror").waitFor(30_000);

    const startedAt = Date.now();
    await tauriPage.evaluate(`(() => {
      const el = document.querySelector(".workspace-content");
      if (!el) return;
      el.scrollTop = el.scrollHeight;
    })()`);
    await new Promise((r) => setTimeout(r, 50));
    await tauriPage.evaluate(`(() => {
      const el = document.querySelector(".workspace-content");
      if (!el) return;
      el.scrollTop = 0;
    })()`);
    const elapsedMs = Date.now() - startedAt;

    test.info().annotations.push({
      type: "perf",
      description: `tauri page-scroll wall=${formatMs(elapsedMs)} (smoke only)`,
    });

    expect(
      elapsedMs,
      `tauri page scroll smoke should finish within ${perfBudgets.pageScrollMs} ms (got ${elapsedMs} ms)`,
    ).toBeLessThanOrEqual(perfBudgets.pageScrollMs);
  });
});
