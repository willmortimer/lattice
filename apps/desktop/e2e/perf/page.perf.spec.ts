import { expect, test } from "@playwright/test";
import { perfBudgets, readNavigationMetrics } from "./budgets";
import { formatMs, openTreePage, waitForShellChrome } from "./helpers";

const HOME_PAGE = "Page: Home.md";
const RESEARCH_PAGE = "Page: Research/Competitor Analysis.md";

test.describe("page open", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/");
    await waitForShellChrome(page);
  });

  test("opens a page from the resource tree within budget", async ({ page }) => {
    const startedAt = Date.now();
    await openTreePage(page, HOME_PAGE);
    await page.locator(".page-editor-content .ProseMirror").waitFor({ state: "visible" });
    await expect(page.getByRole("heading", { name: "Home", exact: true })).toBeVisible();
    const elapsedMs = Date.now() - startedAt;

    const nav = await readNavigationMetrics((fn) => page.evaluate(fn));
    test.info().annotations.push({
      type: "perf",
      description: [
        `page-open wall=${formatMs(elapsedMs)}`,
        `performance.now=${formatMs(nav.performanceNowMs)}`,
      ].join(" "),
    });

    expect(
      elapsedMs,
      `page open should complete within ${perfBudgets.pageOpenMs} ms (got ${elapsedMs} ms)`,
    ).toBeLessThanOrEqual(perfBudgets.pageOpenMs);
  });

  test("scrolls open page content within budget when a second page is available", async ({
    page,
  }) => {
    const researchPage = page.getByRole("button", { name: RESEARCH_PAGE, exact: true });
    if ((await researchPage.count()) === 0) {
      const researchFolder = page.locator(".tree-folder-row").filter({
        has: page.locator(".tree-folder-name", { hasText: /^Research$/ }),
      });
      if ((await researchFolder.count()) > 0) {
        await researchFolder.first().click();
      }
    }

    test.skip(
      (await researchPage.count()) === 0,
      "demo fixture has no Research page in tree",
    );

    await openTreePage(page, RESEARCH_PAGE);
    await page.locator(".page-editor-content .ProseMirror").waitFor({ state: "visible" });

    const scrollTarget = page.locator(".workspace-content");
    const startedAt = Date.now();
    await scrollTarget.evaluate((element) => {
      element.scrollTop = element.scrollHeight;
    });
    await page.waitForTimeout(50);
    await scrollTarget.evaluate((element) => {
      element.scrollTop = 0;
    });
    const elapsedMs = Date.now() - startedAt;

    test.info().annotations.push({
      type: "perf",
      description: `page-scroll wall=${formatMs(elapsedMs)} (demo pages are short; smoke only)`,
    });

    expect(
      elapsedMs,
      `page scroll smoke should finish within ${perfBudgets.pageScrollMs} ms (got ${elapsedMs} ms)`,
    ).toBeLessThanOrEqual(perfBudgets.pageScrollMs);
  });
});
