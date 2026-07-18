import type { Page } from "@playwright/test";
import type { HeapSnapshot } from "./budgets";

export const shellChrome = {
  workspaceTitle: ".workspace-title",
  resourceTree: ".resource-tree-virtual",
  activityRail: '[aria-label="Workspace areas"]',
} as const;

export async function waitForShellChrome(page: Page): Promise<void> {
  await page.locator(shellChrome.workspaceTitle).filter({ hasText: "First Look" }).waitFor({
    state: "visible",
  });
  await page.locator(shellChrome.resourceTree).waitFor({ state: "visible" });
  await page.locator(shellChrome.activityRail).waitFor({ state: "visible" });
}

export async function openTreePage(page: Page, label: string): Promise<void> {
  const button = page.getByRole("button", { name: label, exact: true });
  await button.scrollIntoViewIfNeeded();
  await button.click();
}

export async function readJsHeap(page: Page): Promise<HeapSnapshot | null> {
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
