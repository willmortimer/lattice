import type { Page } from "@playwright/test";

import { waitForShellChrome } from "./helpers";

export const agentPerfSelectors = {
  showAgentButton: 'button[aria-label="Show agent"]',
  workbenchLayoutButton: 'button:has-text("Workbench")',
  threadViewport: ".agent-thread-viewport",
  workbenchResizeHandle: ".agent-workbench-resize-handle",
  workbenchGroup: "#agent-workbench",
} as const;

type PerfHarnessBridge = {
  version: number;
  prepareAgentWorkbench: () => void;
  canInjectAgentMessages: () => boolean;
  injectAgentMessages: (count: number) => Promise<boolean>;
};

async function readPerfHarness(page: Page): Promise<PerfHarnessBridge | null> {
  return page.evaluate(() => {
    const harness = window.__latticePerfHarness;
    if (!harness || harness.version !== 1) {
      return null;
    }
    return {
      version: harness.version,
      prepareAgentWorkbench: harness.prepareAgentWorkbench,
      canInjectAgentMessages: harness.canInjectAgentMessages,
      injectAgentMessages: harness.injectAgentMessages,
    };
  });
}

export async function prepareAgentWorkbench(page: Page): Promise<boolean> {
  const harness = await readPerfHarness(page);
  if (!harness) {
    return false;
  }
  await page.evaluate(() => {
    window.__latticePerfHarness?.prepareAgentWorkbench();
  });
  await page.locator(agentPerfSelectors.workbenchGroup).waitFor({
    state: "visible",
    timeout: 15_000,
  });
  return true;
}

export async function openAgentWorkbenchFromShell(page: Page): Promise<boolean> {
  await page.goto("/");
  await waitForShellChrome(page);

  const harnessReady = await prepareAgentWorkbench(page);
  if (harnessReady) {
    return true;
  }

  await page.locator(agentPerfSelectors.showAgentButton).click();
  await page.locator(agentPerfSelectors.workbenchLayoutButton).click();
  await page.locator(agentPerfSelectors.workbenchGroup).waitFor({
    state: "visible",
    timeout: 15_000,
  });
  return true;
}

export async function canInjectAgentMessages(page: Page): Promise<boolean> {
  return page.evaluate(() => window.__latticePerfHarness?.canInjectAgentMessages() === true);
}

export async function injectAgentMessages(page: Page, count: number): Promise<boolean> {
  return page.evaluate(async (messageCount) => {
    const harness = window.__latticePerfHarness;
    if (!harness) {
      return false;
    }
    return harness.injectAgentMessages(messageCount);
  }, count);
}

export async function scrollAgentThread(page: Page): Promise<void> {
  const viewport = page.locator(agentPerfSelectors.threadViewport);
  await viewport.evaluate((element) => {
    element.scrollTop = 0;
  });
  await viewport.evaluate((element) => {
    element.scrollTop = element.scrollHeight;
  });
  await page.waitForTimeout(50);
  await viewport.evaluate((element) => {
    element.scrollTop = 0;
  });
}

export async function dragWorkbenchResizeHandle(page: Page, deltaX: number): Promise<void> {
  const handle = page.locator(agentPerfSelectors.workbenchResizeHandle);
  await handle.waitFor({ state: "visible", timeout: 15_000 });
  const box = await handle.boundingBox();
  if (!box) {
    throw new Error("agent workbench resize handle is not visible");
  }
  const startX = box.x + box.width / 2;
  const startY = box.y + box.height / 2;
  await page.mouse.move(startX, startY);
  await page.mouse.down();
  await page.mouse.move(startX + deltaX, startY, { steps: 8 });
  await page.mouse.up();
}
