import { describe, expect, it } from "vitest";

import { BLOCK_DRAG_MIME, BlockDragHandle } from "./BlockDragHandle";
import { liveEditorExtensions } from "./richEditorExtensions";

describe("BlockDragHandle", () => {
  it("exports a stable MIME type for dragover type checks", () => {
    expect(BLOCK_DRAG_MIME).toBe("application/x-lattice-block-pos");
  });

  it("is registered on the live editor extension list", () => {
    expect(BlockDragHandle.name).toBe("blockDragHandle");
    expect(liveEditorExtensions.some((extension) => extension.name === "blockDragHandle")).toBe(true);
  });
});
