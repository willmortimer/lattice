import { expect, test } from "@playwright/test";

import { dragWorkbenchResizeHandle, openAgentWorkbenchFromShell } from "./agentHarness";
import { perfBudgets } from "./budgets";
import { formatMs } from "./helpers";

test.describe("agent workbench layout", () => {
  test("drags the workbench resize handle within soft budget", async ({ page }) => {
    await openAgentWorkbenchFromShell(page);

    const startedAt = Date.now();
    await dragWorkbenchResizeHandle(page, 80);
    await dragWorkbenchResizeHandle(page, -40);
    const elapsedMs = Date.now() - startedAt;

    test.info().annotations.push({
      type: "perf",
      description: `agent-workbench-resize wall=${formatMs(elapsedMs)}`,
    });

    expect(
      elapsedMs,
      `workbench resize smoke should finish within ${perfBudgets.agentWorkbenchResizeMs} ms (got ${elapsedMs} ms)`,
    ).toBeLessThanOrEqual(perfBudgets.agentWorkbenchResizeMs);
  });
});
