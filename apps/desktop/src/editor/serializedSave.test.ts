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

  it("makes exactly one save attempt on generic failure without spinning", async () => {
    let saveCount = 0;
    const statuses: string[] = [];

    const controller = createSerializedSaveController({
      initialRevision: null as string | null,
      save: async () => {
        saveCount += 1;
        throw new Error("disk full");
      },
      onStatus: (status) => statuses.push(status),
      savedIndicatorMs: 0,
    });

    controller.markDirty(0);
    await vi.advanceTimersByTimeAsync(0);
    await Promise.resolve();
    await Promise.resolve();

    expect(saveCount).toBe(1);
    expect(controller.isFailed()).toBe(true);
    expect(statuses).toContain("error");

    await controller.flush();
    await Promise.resolve();
    expect(saveCount).toBe(1);
    controller.dispose();
  });

  it("retry clears failure latch and saves again", async () => {
    let saveCount = 0;
    const statuses: string[] = [];

    const controller = createSerializedSaveController({
      initialRevision: null as string | null,
      save: async () => {
        saveCount += 1;
        if (saveCount === 1) throw new Error("temporary");
        return "rev-1";
      },
      onStatus: (status) => statuses.push(status),
      savedIndicatorMs: 0,
    });

    controller.markDirty(0);
    await vi.advanceTimersByTimeAsync(0);
    await Promise.resolve();
    await Promise.resolve();
    expect(saveCount).toBe(1);
    expect(controller.isFailed()).toBe(true);

    await controller.retry();
    await Promise.resolve();
    await Promise.resolve();
    expect(saveCount).toBe(2);
    expect(controller.isFailed()).toBe(false);
    expect(statuses.at(-1)).toBe("saved");
    controller.dispose();
  });

  it("records dirty after failure without scheduling another save", async () => {
    let saveCount = 0;
    const statuses: string[] = [];

    const controller = createSerializedSaveController({
      initialRevision: null as string | null,
      save: async () => {
        saveCount += 1;
        throw new Error("fail");
      },
      onStatus: (status) => statuses.push(status),
      savedIndicatorMs: 0,
    });

    controller.markDirty(0);
    await vi.advanceTimersByTimeAsync(0);
    await Promise.resolve();
    await Promise.resolve();

    controller.markDirty(50);
    expect(statuses.filter((s) => s === "dirty")).toHaveLength(2);

    await vi.advanceTimersByTimeAsync(50);
    await Promise.resolve();
    await Promise.resolve();

    expect(saveCount).toBe(1);
    expect(controller.isFailed()).toBe(true);
    controller.dispose();
  });

  it("does not auto-retry after conflict", async () => {
    let saveCount = 0;
    const conflict = new Error("stale");
    conflict.name = "StaleRevisionError";
    const statuses: string[] = [];

    const controller = createSerializedSaveController({
      initialRevision: "rev-0",
      save: async () => {
        saveCount += 1;
        throw conflict;
      },
      onStatus: (status) => statuses.push(status),
      savedIndicatorMs: 0,
    });

    controller.markDirty(0);
    await vi.advanceTimersByTimeAsync(0);
    await Promise.resolve();
    await Promise.resolve();

    expect(saveCount).toBe(1);
    expect(controller.isConflicted()).toBe(true);
    expect(statuses).toContain("conflict");

    controller.markDirty(50);
    await vi.advanceTimersByTimeAsync(50);
    await controller.flush();
    await Promise.resolve();

    expect(saveCount).toBe(1);
    controller.dispose();
  });

  it("does not emit status after dispose while save is in flight", async () => {
    let resolveSave!: (revision: string) => void;
    const statuses: string[] = [];
    let onRevisionCount = 0;

    const controller = createSerializedSaveController({
      initialRevision: null as string | null,
      save: () =>
        new Promise((resolve) => {
          resolveSave = resolve;
        }),
      onStatus: (status) => statuses.push(status),
      onRevision: () => {
        onRevisionCount += 1;
      },
      savedIndicatorMs: 0,
    });

    controller.markDirty(0);
    await vi.advanceTimersByTimeAsync(0);
    expect(statuses).toContain("saving");

    const statusCountBeforeDispose = statuses.length;
    controller.dispose();

    resolveSave("rev-1");
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();

    expect(statuses.length).toBe(statusCountBeforeDispose);
    expect(onRevisionCount).toBe(0);
    expect(statuses).not.toContain("saved");
  });

  it("ignores in-flight save success after dispose (Quick Note discard race)", async () => {
    let saveCount = 0;
    let resolveSave!: (revision: string) => void;
    const statuses: string[] = [];
    let onRevisionCount = 0;

    const controller = createSerializedSaveController({
      initialRevision: null as string | null,
      save: () => {
        saveCount += 1;
        return new Promise((resolve) => {
          resolveSave = resolve;
        });
      },
      onStatus: (status) => statuses.push(status),
      onRevision: () => {
        onRevisionCount += 1;
      },
      savedIndicatorMs: 0,
    });

    controller.markDirty(0);
    await vi.advanceTimersByTimeAsync(0);
    expect(saveCount).toBe(1);

    controller.dispose();
    resolveSave("rev-1");
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();

    expect(onRevisionCount).toBe(0);
    expect(statuses).not.toContain("saved");
    expect(statuses).not.toContain("idle");
  });
});
