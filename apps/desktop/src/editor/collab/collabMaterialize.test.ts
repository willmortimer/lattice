import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { parseMarkdownToJSON } from "../markdown";
import { StaleRevisionError } from "../pageIO";
import { createSerializedSaveController } from "../serializedSave";
import type { PageIO } from "../pageIO";
import {
  buildMaterializedPageRaw,
  materializeCollabPage,
  shouldScheduleCollabCheckpoint,
} from "./collabMaterialize";

const SAMPLE_JSON = parseMarkdownToJSON("# Title\n\nBody text.\n");

describe("collabMaterialize", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("shouldScheduleCollabCheckpoint is true only in collaborative mode", () => {
    expect(shouldScheduleCollabCheckpoint("plain")).toBe(false);
    expect(shouldScheduleCollabCheckpoint("collaborative")).toBe(true);
  });

  it("buildMaterializedPageRaw preserves frontmatter and serializes edit JSON", () => {
    const raw = buildMaterializedPageRaw({
      frontmatter: "---\ntitle: Note\n---\n",
      mode: "edit",
      draftBody: "ignored in edit mode",
      editJson: SAMPLE_JSON,
    });

    expect(raw.startsWith("---\ntitle: Note\n---\n")).toBe(true);
    expect(raw).toContain("# Title");
    expect(raw).toContain("Body text.");
    expect(raw).not.toContain("ignored in edit mode");
  });

  it("materializeCollabPage calls PageIO.save with markdown and base revision", async () => {
    const save = vi.fn(async () => "rev-materialized");
    const io: PageIO = {
      load: async () => ({ raw: "", revision: "rev-0" }),
      save,
    };

    const revision = await materializeCollabPage(
      {
        getFrontmatter: () => null,
        getMode: () => "edit",
        getDraftBody: () => "",
        getEditJson: () => SAMPLE_JSON,
        io,
      },
      "rev-0",
    );

    expect(revision).toBe("rev-materialized");
    expect(save).toHaveBeenCalledOnce();
    const [raw, baseRevision] = save.mock.calls[0] ?? [];
    expect(baseRevision).toBe("rev-0");
    expect(raw).toContain("# Title");
    expect(raw).toContain("Body text.");
  });

  it("checkpoint controller debounces materialize — keystrokes do not save immediately", async () => {
    const save = vi.fn(async () => "rev-1");
    const controller = createSerializedSaveController({
      initialRevision: "rev-0",
      save: async (baseRevision) => {
        await save(baseRevision);
        return "rev-1";
      },
      onStatus: () => undefined,
      savedIndicatorMs: 0,
    });

    controller.markDirty(500);
    controller.markDirty(500);
    controller.markDirty(500);
    expect(save).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(500);
    await Promise.resolve();
    await Promise.resolve();

    expect(save).toHaveBeenCalledOnce();
    expect(save.mock.calls[0]?.[0]).toBe("rev-0");
    controller.dispose();
  });

  it("does not save on markDirty before debounce elapses", async () => {
    let saveCount = 0;
    const controller = createSerializedSaveController({
      initialRevision: null,
      save: async () => {
        saveCount += 1;
        return "rev-1";
      },
      onStatus: () => undefined,
      savedIndicatorMs: 0,
    });

    controller.markDirty(800);
    await vi.advanceTimersByTimeAsync(100);
    expect(saveCount).toBe(0);

    await vi.advanceTimersByTimeAsync(700);
    await Promise.resolve();
    await Promise.resolve();
    expect(saveCount).toBe(1);
    controller.dispose();
  });

  it("StaleRevisionError during materialize latches conflict and blocks further checkpoints", async () => {
    let saveCount = 0;
    const statuses: string[] = [];
    const controller = createSerializedSaveController({
      initialRevision: "rev-0",
      save: async () => {
        saveCount += 1;
        throw new StaleRevisionError("disk revision moved");
      },
      onStatus: (status) => {
        statuses.push(status);
      },
      isConflict: (error) => error instanceof StaleRevisionError,
      savedIndicatorMs: 0,
    });

    controller.markDirty(0);
    await vi.runAllTimersAsync();
    await Promise.resolve();

    expect(saveCount).toBe(1);
    expect(controller.isConflicted()).toBe(true);
    expect(statuses).toContain("conflict");

    controller.markDirty(500);
    await vi.advanceTimersByTimeAsync(500);
    await Promise.resolve();
    expect(saveCount).toBe(1);
    controller.dispose();
  });
});
