import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { persistActiveAgentRun } from "../lib/agentActiveRun";
import {
  hasPersistedActiveAgentRun,
  reconnectPersistedActiveAgentRun,
} from "./agentReconnect";

describe("agentReconnect", () => {
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

  it("hasPersistedActiveAgentRun is false when sessionStorage is empty", () => {
    expect(hasPersistedActiveAgentRun("/tmp/ws", "thread-1")).toBe(false);
  });

  it("reconnectPersistedActiveAgentRun invokes resumeStream when a run is persisted", async () => {
    persistActiveAgentRun({
      workspaceRoot: "/tmp/ws",
      threadId: "thread-1",
      runId: "run-1",
      afterSequence: 2,
    });
    const resumeStream = vi.fn(async () => {});

    await expect(
      reconnectPersistedActiveAgentRun("/tmp/ws", "thread-1", resumeStream),
    ).resolves.toBe(true);
    expect(resumeStream).toHaveBeenCalledOnce();
  });

  it("reconnectPersistedActiveAgentRun skips resumeStream when no active run", async () => {
    const resumeStream = vi.fn(async () => {});

    await expect(
      reconnectPersistedActiveAgentRun("/tmp/ws", "thread-1", resumeStream),
    ).resolves.toBe(false);
    expect(resumeStream).not.toHaveBeenCalled();
  });
});
