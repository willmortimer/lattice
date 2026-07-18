import type { Page } from "@playwright/test";
import type { BrowserPageAdapter, TauriPage } from "@srsholmes/tauri-playwright";
import type { HeapSnapshot } from "./budgets";

export const shellChrome = {
  workspaceTitle: ".workspace-title",
  resourceTree: ".resource-tree-virtual",
  activityRail: '[aria-label="Workspace areas"]',
} as const;

/** Page surface shared by browser Playwright and tauri-plugin-playwright adapters. */
export type PerfPage = Page | TauriPage | BrowserPageAdapter;

type WaitableLocator = {
  filter(options: { hasText?: string | RegExp }): WaitableLocator;
  waitFor(options?: { state?: "visible" | "attached"; timeout?: number } | number): Promise<void>;
  scrollIntoViewIfNeeded(): Promise<void>;
  click(): Promise<void>;
};

function isPlaywrightPage(page: PerfPage): page is Page {
  return typeof (page as Page).context === "function";
}

async function waitVisible(locator: WaitableLocator, tauri: boolean): Promise<void> {
  if (tauri) {
    await locator.waitFor(30_000);
    return;
  }
  await locator.waitFor({ state: "visible", timeout: 30_000 });
}

export async function waitForShellChrome(page: PerfPage): Promise<void> {
  const tauri = !isPlaywrightPage(page);
  await waitVisible(
    page.locator(shellChrome.workspaceTitle).filter({ hasText: "First Look" }) as WaitableLocator,
    tauri,
  );
  await waitVisible(page.locator(shellChrome.resourceTree) as WaitableLocator, tauri);
  await waitVisible(page.locator(shellChrome.activityRail) as WaitableLocator, tauri);
}

export async function openTreePage(page: PerfPage, label: string): Promise<void> {
  // Rows virtualize against `.resource-list` (not `.resource-tree-virtual`).
  // Root files (e.g. Home.md) sort after folders, so they often start unmounted.
  if (isPlaywrightPage(page)) {
    const scrollParent = page.locator(".resource-list");
    const button = page.getByRole("button", { name: label, exact: true });
    await scrollParent.evaluate((el) => {
      el.scrollTop = 0;
    });
    for (let i = 0; i < 60; i++) {
      if ((await button.count()) > 0) break;
      await scrollParent.evaluate((el) => {
        el.scrollTop += Math.max(120, el.clientHeight * 0.75);
      });
      await page.waitForTimeout(30);
    }
    await button.scrollIntoViewIfNeeded();
    await button.click();
    return;
  }

  page.setDefaultTimeout(30_000);
  const selector = `[aria-label=${JSON.stringify(label)}]`;
  for (let i = 0; i < 60; i++) {
    const mounted = await page.evaluate(
      `!!document.querySelector(${JSON.stringify(selector)})`,
    );
    if (mounted) break;
    await page.evaluate(`(() => {
      const el = document.querySelector(".resource-list");
      if (!el) return;
      if (${i} === 0) el.scrollTop = 0;
      else el.scrollTop += Math.max(120, el.clientHeight * 0.75);
    })()`);
    await new Promise((r) => setTimeout(r, 40));
  }
  await page.click(selector, { timeout: 30_000 });
}

export async function readJsHeap(page: PerfPage): Promise<HeapSnapshot | null> {
  if (!isPlaywrightPage(page)) return null;
  const client = await page.context().newCDPSession(page);
  try {
    const { usedSize, totalSize } = await client.send("Runtime.getHeapUsage");
    return {
      usedJsHeapBytes: usedSize,
      totalJsHeapBytes: totalSize,
    };
  } catch {
    return null;
  } finally {
    await client.detach();
  }
}

export function formatMs(ms: number): string {
  return `${ms.toFixed(1)} ms`;
}

export function formatMiB(bytes: number): string {
  return `${(bytes / (1024 * 1024)).toFixed(2)} MiB`;
}
