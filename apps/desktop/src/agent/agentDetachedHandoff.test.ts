import { afterEach, beforeEach, describe, expect, it } from "vitest";

import {
  ACTIVE_RUN_STORAGE_KEY,
  clearActiveAgentRun,
  persistActiveAgentRun,
} from "../lib/agentActiveRun";
import {
  AGENT_DETACHED_HANDOFF_KEY,
  applyAgentDetachedHandoffToSession,
  buildAgentDetachedHandoff,
  clearAgentDetachedHandoff,
  parseAgentDetachedHandoff,
  readAgentDetachedHandoff,
  refreshAgentDetachedHandoffActiveRun,
  writeAgentDetachedHandoff,
} from "./agentDetachedHandoff";

function installMemoryStorage(name: "localStorage" | "sessionStorage"): Map<string, string> {
  const memory = new Map<string, string>();
  Object.defineProperty(globalThis, name, {
    configurable: true,
    value: {
      getItem: (key: string) => memory.get(key) ?? null,
      setItem: (key: string, value: string) => {
        memory.set(key, value);
      },
      removeItem: (key: string) => {
        memory.delete(key);
      },
      clear: () => {
        memory.clear();
      },
    },
  });
  return memory;
}

describe("agentDetachedHandoff", () => {
  beforeEach(() => {
    installMemoryStorage("localStorage");
    installMemoryStorage("sessionStorage");
  });

  afterEach(() => {
    Reflect.deleteProperty(globalThis, "localStorage");
    Reflect.deleteProperty(globalThis, "sessionStorage");
  });

  it("parseAgentDetachedHandoff accepts valid payloads and rejects junk", () => {
    expect(parseAgentDetachedHandoff(null)).toBeNull();
    expect(parseAgentDetachedHandoff("{")).toBeNull();
    expect(
      parseAgentDetachedHandoff(
        JSON.stringify({
          workspaceRoot: "/tmp/ws",
          threadId: "thread-1",
          returnLayoutMode: "dock",
          activeRun: null,
        }),
      ),
    ).toEqual({
      workspaceRoot: "/tmp/ws",
      threadId: "thread-1",
      returnLayoutMode: "dock",
      activeRun: null,
    });
    expect(
      parseAgentDetachedHandoff(
        JSON.stringify({
          workspaceRoot: "/tmp/ws",
          threadId: "thread-1",
          returnLayoutMode: "detached",
        }),
      ),
    ).toBeNull();
  });

  it("buildAgentDetachedHandoff copies session active-run into shared handoff", () => {
    persistActiveAgentRun({
      workspaceRoot: "/tmp/ws",
      threadId: "thread-1",
      runId: "run-9",
      afterSequence: 4,
    });
    const handoff = buildAgentDetachedHandoff({
      workspaceRoot: "/tmp/ws",
      threadId: "thread-1",
      returnLayoutMode: "workbench",
    });
    expect(handoff.activeRun).toEqual({
      workspaceRoot: "/tmp/ws",
      threadId: "thread-1",
      runId: "run-9",
      afterSequence: 4,
    });
  });

  it("applyAgentDetachedHandoffToSession seeds sessionStorage for reconnect owner", () => {
    applyAgentDetachedHandoffToSession({
      workspaceRoot: "/tmp/ws",
      threadId: "thread-1",
      returnLayoutMode: "dock",
      activeRun: {
        workspaceRoot: "/tmp/ws",
        threadId: "thread-1",
        runId: "run-2",
        afterSequence: 1,
      },
    });
    expect(sessionStorage.getItem(`${ACTIVE_RUN_STORAGE_KEY}:/tmp/ws:thread-1`)).toContain(
      "run-2",
    );
  });

  it("refreshAgentDetachedHandoffActiveRun writes localStorage and clears when idle", () => {
    writeAgentDetachedHandoff({
      workspaceRoot: "/tmp/ws",
      threadId: "thread-1",
      returnLayoutMode: "dock",
      activeRun: {
        workspaceRoot: "/tmp/ws",
        threadId: "thread-1",
        runId: "stale",
        afterSequence: 0,
      },
    });
    clearActiveAgentRun("/tmp/ws", "thread-1");
    const refreshed = refreshAgentDetachedHandoffActiveRun({
      workspaceRoot: "/tmp/ws",
      threadId: "thread-1",
      returnLayoutMode: "dock",
      activeRun: null,
    });
    expect(refreshed.activeRun).toBeNull();
    expect(readAgentDetachedHandoff()?.activeRun ?? null).toBeNull();
    expect(localStorage.getItem(AGENT_DETACHED_HANDOFF_KEY)).toContain("thread-1");
    clearAgentDetachedHandoff();
    expect(readAgentDetachedHandoff()).toBeNull();
  });
});
