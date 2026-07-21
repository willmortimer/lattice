import type { Page } from "@playwright/test";
import type { BrowserPageAdapter, TauriPage } from "@srsholmes/tauri-playwright";
import type { HeapSnapshot } from "./budgets";

export const shellChrome = {
  workspaceTitle: ".workspace-title",
  resourceTree: ".resource-tree-virtual",
  activityRail: '[aria-label="Workspace areas"]',
} as const;

/**
 * Stable selectors for First Look tree / undo / move / trash Tauri smoke.
 * Folder + file rows use `KIND_LABELS: path` aria-labels from ResourceTree.
 */
export const treeSmoke = {
  /** Toolbar FolderPlus — label is `New folder in <parent>`. */
  newFolderToolbar: 'button[aria-label^="New folder in "]',
  /** Command palette entry wired to `undo_last`. */
  undoPaletteLabel: "Undo last change",
  linkRepairTitle: "#link-repair-title",
  linkRepairPanel: ".link-repair-panel",
  /** Single-path move accept. */
  renameAndRepair: "Rename and repair",
  /** Batch move accept. */
  moveAndRepair: "Move and repair",
  linkRepairDefer: "Defer",
  /** `window.confirm` copy from `handleDeleteResources`. */
  trashConfirmRe: /Delete .+This moves .+ to Trash\./,
  folderLabel: (path: string) => `Folder: ${path}`,
  pageLabel: (path: string) => `Page: ${path}`,
  folderSelector: (path: string) =>
    `[aria-label=${JSON.stringify(`Folder: ${path}`)}]`,
  pageSelector: (path: string) =>
    `[aria-label=${JSON.stringify(`Page: ${path}`)}]`,
} as const;

/**
 * Stable selectors for First Look schema / tabular-import Tauri smoke (P2S02).
 * Prefer promote of `Data/sample.csv` over the native file-picker Import path.
 */
export const schemaSmoke = {
  crmTreeLabel: "Data app: CRM.data",
  sampleCsvLabel: "File: Data/sample.csv",
  addColumnPanel: '[aria-label="Add column"]',
  addColumnNameInput: '[aria-label="Add column"] input[placeholder="column_name"]',
  addColumnTypeSelect: '[aria-label="Add column"] select',
  addColumnSubmit: '[aria-label="Add column"] button.primary-button',
  /** Fresh text column name not present on the CRM template. */
  smokeColumnName: "smoke_notes",
  createTableFromCsv: "Create table from CSV…",
  importReviewPanel: ".csv-import-review-panel",
  importReviewTitle: "#tabular-import-review-title",
  importReviewTitleText: "Review CSV import",
  /** Package name accepted via `window.prompt` during promote. */
  smokeImportPackage: "SmokeImport",
  smokeImportTreeLabel: "Data app: SmokeImport.data",
  smokeImportTitle: "SmokeImport",
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

/** Scroll the virtualized resource list until `aria-label` is mounted. */
export async function scrollTreeUntilLabel(page: PerfPage, label: string): Promise<void> {
  const selector = `[aria-label=${JSON.stringify(label)}]`;
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
    return;
  }

  page.setDefaultTimeout(30_000);
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
}

export async function openTreePage(page: PerfPage, label: string): Promise<void> {
  // Rows virtualize against `.resource-list` (not `.resource-tree-virtual`).
  // Root files (e.g. Home.md) sort after folders, so they often start unmounted.
  await scrollTreeUntilLabel(page, label);
  if (isPlaywrightPage(page)) {
    await page.getByRole("button", { name: label, exact: true }).click();
    return;
  }
  await page.click(`[aria-label=${JSON.stringify(label)}]`, { timeout: 30_000 });
}

function asTauriPage(page: PerfPage): TauriPage | BrowserPageAdapter | null {
  return isPlaywrightPage(page) ? null : page;
}

/** Click a tree row with ⌘/Ctrl so multi-select toggles (Tauri synthetic click). */
export async function metaClickTreeLabel(page: PerfPage, label: string): Promise<void> {
  await scrollTreeUntilLabel(page, label);
  const selector = `[aria-label=${JSON.stringify(label)}]`;
  const tauri = asTauriPage(page);
  if (!tauri) {
    await page.getByRole("button", { name: label, exact: true }).click({
      modifiers: ["Meta"],
    });
    return;
  }
  await tauri.evaluate(`(() => {
    const el = document.querySelector(${JSON.stringify(selector)});
    if (!el) throw new Error("tree row not mounted: " + ${JSON.stringify(label)});
    el.dispatchEvent(new MouseEvent("click", {
      bubbles: true,
      cancelable: true,
      metaKey: true,
      ctrlKey: false,
    }));
  })()`);
}

/**
 * Create a folder via the sidebar New folder toolbar + `window.prompt`.
 * Requires `installDialogHandler({ defaultPromptText })` beforehand on Tauri.
 */
export async function createFolderViaToolbar(
  page: PerfPage,
  parentFolderPath: string,
): Promise<void> {
  await scrollTreeUntilLabel(page, treeSmoke.folderLabel(parentFolderPath));
  if (isPlaywrightPage(page)) {
    await page.getByRole("button", { name: treeSmoke.folderLabel(parentFolderPath), exact: true }).click();
    page.once("dialog", (dialog) => {
      void dialog.accept("Smoke Folder");
    });
    await page.locator(treeSmoke.newFolderToolbar).click();
    return;
  }
  await page.click(treeSmoke.folderSelector(parentFolderPath), { timeout: 30_000 });
  await page.click(treeSmoke.newFolderToolbar, { timeout: 30_000 });
}

/** Workspace undo via ⌘Z / Ctrl+Z (not editable targets). */
export async function undoWorkspaceChange(page: PerfPage): Promise<void> {
  const tauri = asTauriPage(page);
  if (!tauri) {
    await page.keyboard.press("Meta+Z");
    return;
  }
  // Focus shell chrome so Mod+Z is not swallowed by an editor.
  await tauri.click(shellChrome.workspaceTitle, { timeout: 30_000 });
  await tauri.keyboard.press("Meta+Z");
}

/**
 * Drag a page onto a folder row (HTML5 DnD). Prefer this over native menus.
 * Link-repair modal may appear afterward — call `acceptLinkRepairIfPresent`.
 */
export async function movePageToFolder(
  page: PerfPage,
  pagePath: string,
  folderPath: string,
): Promise<void> {
  const sourceLabel = treeSmoke.pageLabel(pagePath);
  const targetLabel = treeSmoke.folderLabel(folderPath);
  await scrollTreeUntilLabel(page, sourceLabel);
  await scrollTreeUntilLabel(page, targetLabel);
  const source = treeSmoke.pageSelector(pagePath);
  const target = treeSmoke.folderSelector(folderPath);
  const tauri = asTauriPage(page);
  if (!tauri) {
    await page.dragAndDrop(source, target);
    return;
  }
  await tauri.dragAndDrop(source, target, { timeout: 30_000 });
}

/**
 * Accept link-repair when the modal appears; no-op if absent.
 * Returns whether a repair accept control was clicked.
 */
export async function acceptLinkRepairIfPresent(
  page: PerfPage,
  timeoutMs = 8_000,
): Promise<boolean> {
  const tauri = asTauriPage(page);
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (tauri) {
      const open = await tauri.evaluate(
        `!!document.querySelector(${JSON.stringify(treeSmoke.linkRepairPanel)})`,
      );
      if (open) {
        const rename = await tauri.evaluate(
          `!![...document.querySelectorAll("button")].some((b) => b.textContent?.trim() === ${JSON.stringify(treeSmoke.renameAndRepair)})`,
        );
        const label = rename ? treeSmoke.renameAndRepair : treeSmoke.moveAndRepair;
        await tauri.getByRole("button", { name: label }).click();
        await tauri.waitForFunction(
          `!document.querySelector(${JSON.stringify(treeSmoke.linkRepairPanel)})`,
          timeoutMs,
        );
        return true;
      }
    } else {
      const panel = page.locator(treeSmoke.linkRepairPanel);
      if ((await panel.count()) > 0) {
        const rename = page.getByRole("button", { name: treeSmoke.renameAndRepair });
        if ((await rename.count()) > 0) await rename.click();
        else await page.getByRole("button", { name: treeSmoke.moveAndRepair }).click();
        await panel.waitFor({ state: "detached", timeout: timeoutMs });
        return true;
      }
    }
    await new Promise((r) => setTimeout(r, 150));
  }
  return false;
}

/**
 * Multi-select trash via Delete/Backspace on the resource tree + `window.confirm`.
 * Requires `installDialogHandler({ defaultConfirm: true })` on Tauri first.
 * Focus a selected tree row before calling so the shell key handler runs.
 */
export async function trashSelectionWithConfirm(page: PerfPage): Promise<void> {
  const tauri = asTauriPage(page);
  if (!tauri) {
    page.once("dialog", (dialog) => {
      void dialog.accept();
    });
    await page.locator(".resource-tree-virtual button[aria-selected='true']").first().focus();
    await page.keyboard.press("Backspace");
    return;
  }
  // Focus a selected tree row (Delete/Backspace only fires when focus is in the tree).
  await tauri.evaluate(`(() => {
    const el = document.querySelector(".resource-tree-virtual button[aria-selected='true']");
    if (!el) throw new Error("no selected tree row to trash");
    el.focus();
  })()`);
  await tauri.keyboard.press("Backspace");
}

export async function treeLabelMounted(page: PerfPage, label: string): Promise<boolean> {
  const selector = `[aria-label=${JSON.stringify(label)}]`;
  if (isPlaywrightPage(page)) {
    return (await page.locator(selector).count()) > 0;
  }
  return Boolean(
    await page.evaluate(`!!document.querySelector(${JSON.stringify(selector)})`),
  );
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
