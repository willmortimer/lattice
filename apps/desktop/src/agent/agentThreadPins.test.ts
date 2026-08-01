import { afterEach, beforeEach, describe, expect, it } from "vitest";

import {
  PINNED_THREADS_STORAGE_KEY,
  isThreadPinned,
  parsePinnedThreads,
  persistPinnedThreads,
  readPinnedThreads,
  sortThreadsWithPins,
  togglePinnedThreadId,
} from "./agentThreadPins";

function installMemoryStorage(): Map<string, string> {
  const memory = new Map<string, string>();
  Object.defineProperty(globalThis, "localStorage", {
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

describe("agentThreadPins", () => {
  beforeEach(() => {
    installMemoryStorage();
  });

  afterEach(() => {
    Reflect.deleteProperty(globalThis, "localStorage");
  });

  it("parsePinnedThreads accepts workspace maps and rejects junk", () => {
    expect(parsePinnedThreads(null)).toEqual({});
    expect(parsePinnedThreads("{")).toEqual({});
    expect(parsePinnedThreads(JSON.stringify(["thread-1"]))).toEqual({});
    expect(
      parsePinnedThreads(
        JSON.stringify({
          "/tmp/ws/": ["thread-1", "thread-1", "", 12],
          "/other": "nope",
        }),
      ),
    ).toEqual({
      "/tmp/ws": ["thread-1"],
    });
  });

  it("togglePinnedThreadId pins then unpins and drops empty workspaces", () => {
    const pinned = togglePinnedThreadId({}, "/tmp/ws", "thread-1");
    expect(isThreadPinned(pinned, "/tmp/ws/", "thread-1")).toBe(true);
    expect(togglePinnedThreadId(pinned, "/tmp/ws", "thread-1")).toEqual({});
  });

  it("persist + read round-trip through localStorage", () => {
    persistPinnedThreads({ "/tmp/ws": ["a", "b"] });
    expect(localStorage.getItem(PINNED_THREADS_STORAGE_KEY)).toContain("\"a\"");
    expect(readPinnedThreads()).toEqual({ "/tmp/ws": ["a", "b"] });
  });

  it("sortThreadsWithPins keeps pinned threads first then by updatedAt", () => {
    const sorted = sortThreadsWithPins(
      [
        { id: "old", updatedAt: 1 },
        { id: "new", updatedAt: 3 },
        { id: "pinned", updatedAt: 2 },
      ],
      ["pinned"],
    );
    expect(sorted.map((thread) => thread.id)).toEqual(["pinned", "new", "old"]);
  });
});
