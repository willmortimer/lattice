import { describe, expect, it } from "vitest";

import {
  BLOCK_DRAG_MIME,
  BlockDragHandle,
  getActiveBlockDragFromPos,
  setActiveBlockDragFromPos,
} from "./BlockDragHandle";
import { isBlockDragArmed } from "./blockReorder";
import { liveEditorExtensions } from "./richEditorExtensions";

describe("BlockDragHandle", () => {
  it("exports a stable MIME type for drag payloads", () => {
    expect(BLOCK_DRAG_MIME).toBe("application/x-lattice-block-pos");
  });

  it("is registered on the live editor extension list", () => {
    expect(BlockDragHandle.name).toBe("blockDragHandle");
    expect(liveEditorExtensions.some((extension) => extension.name === "blockDragHandle")).toBe(true);
  });

  it("arms dragover from module-level pos without relying on MIME types", () => {
    setActiveBlockDragFromPos(null);
    expect(isBlockDragArmed(getActiveBlockDragFromPos())).toBe(false);

    setActiveBlockDragFromPos(15);
    expect(getActiveBlockDragFromPos()).toBe(15);
    expect(isBlockDragArmed(getActiveBlockDragFromPos())).toBe(true);

    setActiveBlockDragFromPos(null);
    expect(isBlockDragArmed(getActiveBlockDragFromPos())).toBe(false);
  });
});
