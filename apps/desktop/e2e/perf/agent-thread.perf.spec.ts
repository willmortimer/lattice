import { expect, test } from "@playwright/test";

import {
  canInjectAgentMessages,
  injectAgentMessages,
  openAgentWorkbenchFromShell,
  scrollAgentThread,
} from "./agentHarness";
import { perfBudgets } from "./budgets";
import { formatMs } from "./helpers";

const THREAD_MESSAGE_COUNT = 1_000;

test.describe("agent thread scale", () => {
  test("scrolls a 1k-message transcript within soft budget when harness can inject", async ({
    page,
  }) => {
    await openAgentWorkbenchFromShell(page);

    test.skip(
      !(await canInjectAgentMessages(page)),
      "browser demo cannot inject agent transcript messages yet",
    );

    const injected = await injectAgentMessages(page, THREAD_MESSAGE_COUNT);
    test.skip(!injected, "perf harness declined agent message injection");

    await page.locator(".agent-thread-viewport").waitFor({ state: "visible", timeout: 15_000 });

    const startedAt = Date.now();
    await scrollAgentThread(page);
    const elapsedMs = Date.now() - startedAt;

    test.info().annotations.push({
      type: "perf",
      description: `agent-1k-scroll wall=${formatMs(elapsedMs)} messages=${THREAD_MESSAGE_COUNT}`,
    });

    expect(
      elapsedMs,
      `1k-message agent scroll smoke should finish within ${perfBudgets.agentThreadScrollMs} ms (got ${elapsedMs} ms)`,
    ).toBeLessThanOrEqual(perfBudgets.agentThreadScrollMs);
  });
});
