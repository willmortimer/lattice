import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { createSerializedSaveController } from "./serializedSave";

describe("createSerializedSaveController", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("deduplicates dirty status reports while already dirty", () => {
    const statuses: string[] = [];
    const controller = createSerializedSaveController({
      initialRevision: null as string | null,
      save: async () => "rev-1",
      onStatus: (status) => statuses.push(status),
      savedIndicatorMs: 0,
    });

    controller.markDirty(100);
    controller.markDirty(100);
    controller.markDirty(100);
    expect(statuses.filter((s) => s === "dirty")).toHaveLength(1);
    controller.dispose();
  });

  it("requeues edits that arrive while a save is in flight", async () => {
    let saveCount = 0;
    const resolvers: Array<(revision: string) => void> = [];
    const statuses: string[] = [];
    const payloads: Array<string | null> = [];

    const controller = createSerializedSaveController({
      initialRevision: null as string | null,
      save: (revision) => {
        payloads.push(revision);
        saveCount += 1;
        return new Promise((resolve) => {
          resolvers.push(resolve);
        });
      },
      onStatus: (status) => statuses.push(status),
      savedIndicatorMs: 0,
    });

    controller.markDirty(50);
    await vi.advanceTimersByTimeAsync(50);

    expect(saveCount).toBe(1);
    expect(statuses.at(-1)).toBe("saving");

    controller.markDirty(50);
    await vi.advanceTimersByTimeAsync(50);
    expect(saveCount).toBe(1);

    resolvers[0]?.("rev-1");
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();

    expect(saveCount).toBe(2);
    resolvers[1]?.("rev-2");
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();

    expect(statuses.at(-1)).toBe("saved");
    expect(payloads).toEqual([null, "rev-1"]);
    controller.dispose();
  });

  it("coalesces flush calls while a save loop is active", async () => {
    let inflight = 0;
    let maxInflight = 0;
    let resolveSave!: (revision: string) => void;

    const controller = createSerializedSaveController({
      initialRevision: null as string | null,
      save: () => {
        inflight += 1;
        maxInflight = Math.max(maxInflight, inflight);
        return new Promise((resolve) => {
          resolveSave = (revision) => {
            inflight -= 1;
            resolve(revision);
          };
        });
      },
      onStatus: () => undefined,
      savedIndicatorMs: 0,
    });

    controller.markDirty(0);
    await vi.advanceTimersByTimeAsync(0);
    const a = controller.flush();
    const b = controller.flush();
    expect(a).toBe(b);
    expect(maxInflight).toBe(1);
    resolveSave("rev");
    await a;
    controller.dispose();
  });
});
