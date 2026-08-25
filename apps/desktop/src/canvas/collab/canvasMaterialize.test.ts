import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as Y from "yjs";

import { createSerializedSaveController } from "../../editor/serializedSave";
import type { CanvasData } from "../types";
import { CanvasStaleRevisionError } from "../adapter";
import { applyCanvasDataToYDoc } from "./canvasYDoc";
import {
  buildMaterializedCanvasRaw,
  materializeCollabCanvas,
  shouldPatchPlainCanvas,
  shouldScheduleCollabCheckpoint,
  type CanvasFileIO,
} from "./canvasMaterialize";

const SAMPLE: CanvasData = {
  nodes: [
    { id: "note", type: "text", text: "Hello", x: 0, y: 0, width: 120, height: 80 },
    { id: "file", type: "file", file: "Notes/A.md", x: 140, y: 0, width: 120, height: 80 },
    { id: "link", type: "link", url: "https://example.com", x: 0, y: 100, width: 120, height: 80 },
    { id: "group", type: "group", label: "Box", x: -10, y: -10, width: 300, height: 220 },
  ],
  edges: [{ id: "e1", fromNode: "note", toNode: "file", fromSide: "right", toSide: "left" }],
};

describe("canvasMaterialize", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("schedules checkpoints only in collaborative mode", () => {
    expect(shouldScheduleCollabCheckpoint("plain")).toBe(false);
    expect(shouldScheduleCollabCheckpoint("collaborative")).toBe(true);
    expect(shouldPatchPlainCanvas("plain")).toBe(true);
    expect(shouldPatchPlainCanvas("collaborative")).toBe(false);
  });

  it("buildMaterializedCanvasRaw writes pretty JSON Canvas, not a Yrs dump", () => {
    const raw = buildMaterializedCanvasRaw(SAMPLE);
    expect(raw.startsWith("{")).toBe(true);
    const parsed = JSON.parse(raw) as CanvasData;
    expect(parsed).toEqual(SAMPLE);
    expect(raw).toContain("Hello");
    expect(raw).not.toContain("Yjs");
  });

  it("materializeCollabCanvas writes Y.Doc maps as portable JSON", async () => {
    const ydoc = new Y.Doc();
    applyCanvasDataToYDoc(ydoc, SAMPLE);
    const save = vi.fn(async () => "rev-materialized");
    const io: CanvasFileIO = { save };

    const revision = await materializeCollabCanvas({ ydoc, io }, "rev-0");

    expect(revision).toBe("rev-materialized");
    expect(save).toHaveBeenCalledOnce();
    const [raw, baseRevision] = save.mock.calls[0] ?? [];
    expect(baseRevision).toBe("rev-0");
    expect(JSON.parse(String(raw))).toEqual(SAMPLE);
  });

  it("checkpoint controller debounces materialize — gestures do not save immediately", async () => {
    const save = vi.fn(async () => "rev-1");
    const ydoc = new Y.Doc();
    applyCanvasDataToYDoc(ydoc, SAMPLE);
    const controller = createSerializedSaveController({
      initialRevision: "rev-0",
      save: async (baseRevision) => {
        await materializeCollabCanvas(
          {
            ydoc,
            io: {
              save: async () => {
                save();
                return "rev-1";
              },
            },
          },
          baseRevision,
        );
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
    controller.dispose();
  });

  it("CanvasStaleRevisionError during materialize latches conflict", async () => {
    let saveCount = 0;
    const statuses: string[] = [];
    const controller = createSerializedSaveController({
      initialRevision: "rev-0",
      save: async () => {
        saveCount += 1;
        throw new CanvasStaleRevisionError("disk revision moved");
      },
      onStatus: (status) => {
        statuses.push(status);
      },
      isConflict: (error) => error instanceof CanvasStaleRevisionError,
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
