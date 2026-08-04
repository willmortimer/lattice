import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../demo", () => ({
  inBrowser: false,
}));

describe("agent run query options", () => {
  beforeEach(() => {
    vi.resetModules();
  });

  it("builds enabled options with stable keys and no focus refetch", async () => {
    const { queryKeys } = await import("./keys");
    const { agentRunStatusQueryOptions, agentRunActiveQueryOptions } = await import(
      "./useAgentRunStatusQuery"
    );
    const { agentRunEventsQueryOptions } = await import("./useAgentRunEventsQuery");

    const status = agentRunStatusQueryOptions("/ws", "run-1");
    expect(status.queryKey).toEqual(queryKeys.agentRunStatus("/ws", "run-1"));
    expect(status.enabled).toBe(true);
    expect(status.refetchOnWindowFocus).toBe(false);

    const events = agentRunEventsQueryOptions("/ws", "run-1");
    expect(events.queryKey).toEqual(queryKeys.agentRunEvents("/ws", "run-1"));
    expect(events.enabled).toBe(true);
    expect(events.refetchOnWindowFocus).toBe(false);

    const active = agentRunActiveQueryOptions("/ws", "thread-1");
    expect(active.queryKey).toEqual(queryKeys.agentRunActive("/ws", "thread-1"));
    expect(active.enabled).toBe(true);
    expect(active.refetchOnWindowFocus).toBe(false);
  });
});
