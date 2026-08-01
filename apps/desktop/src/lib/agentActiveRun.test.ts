import { afterEach, beforeEach, describe, expect, it } from "vitest";

import {
  ACTIVE_RUN_STORAGE_KEY,
  activeRunStorageKey,
  clearActiveAgentRun,
  loadActiveAgentRun,
  persistActiveAgentRun,
  updateActiveAgentRunSequence,
} from "./agentActiveRun";

describe("agentActiveRun", () => {
  const memory = new Map<string, string>();

  beforeEach(() => {
    memory.clear();
    Object.defineProperty(globalThis, "sessionStorage", {
      configurable: true,
      value: {
        getItem: (key: string) => memory.get(key) ?? null,
        setItem: (key: string, value: string) => {
          memory.set(key, value);
        },
        removeItem: (key: string) => {
          memory.delete(key);
        },
      },
    });
  });

  afterEach(() => {
    Reflect.deleteProperty(globalThis, "sessionStorage");
  });

  it("persists and loads an active run reference", () => {
    persistActiveAgentRun({
      workspaceRoot: "/tmp/ws",
      threadId: "thread-1",
      runId: "run-1",
      afterSequence: 3,
    });

    expect(loadActiveAgentRun("/tmp/ws", "thread-1")).toEqual({
      workspaceRoot: "/tmp/ws",
      threadId: "thread-1",
      runId: "run-1",
      afterSequence: 3,
    });
    expect(activeRunStorageKey("/tmp/ws", "thread-1")).toContain(ACTIVE_RUN_STORAGE_KEY);
  });

  it("advances the ack cursor monotonically", () => {
    persistActiveAgentRun({
      workspaceRoot: "/tmp/ws",
      threadId: "thread-1",
      runId: "run-1",
      afterSequence: 2,
    });
    updateActiveAgentRunSequence("/tmp/ws", "thread-1", 1);
    expect(loadActiveAgentRun("/tmp/ws", "thread-1")?.afterSequence).toBe(2);
    updateActiveAgentRunSequence("/tmp/ws", "thread-1", 5);
    expect(loadActiveAgentRun("/tmp/ws", "thread-1")?.afterSequence).toBe(5);
  });

  it("clears the active run reference", () => {
    persistActiveAgentRun({
      workspaceRoot: "/tmp/ws",
      threadId: "thread-1",
      runId: "run-1",
      afterSequence: 0,
    });
    clearActiveAgentRun("/tmp/ws", "thread-1");
    expect(loadActiveAgentRun("/tmp/ws", "thread-1")).toBeNull();
  });
});
