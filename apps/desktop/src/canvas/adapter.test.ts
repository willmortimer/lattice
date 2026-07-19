import { describe, expect, it } from "vitest";
import {
  canvasOutline,
  canvasRelativePath,
  keyboardMoveDelta,
  previewAddEdge,
  previewAddTextNode,
  previewMoveNodes,
  previewPlaceResource,
  previewRemoveEdges,
  previewRemoveNodes,
  previewResizeNodes,
  previewUpdateTextNode,
} from "./adapter";
import type { CanvasData } from "./types";

const data: CanvasData = {
  nodes: [
    { id: "a", type: "text", text: "Alpha", x: 1, y: 2, width: 100, height: 80 },
    { id: "b", type: "file", file: "Notes/B.md", x: 4, y: 5, width: 100, height: 80 },
  ],
  edges: [{ id: "ab", fromNode: "a", toNode: "b" }],
};

describe("canvasRelativePath", () => {
  it("writes references relative to the canvas parent", () => {
    expect(canvasRelativePath("Canvases/Board.canvas", "Product/Vision.md")).toBe("../Product/Vision.md");
    expect(canvasRelativePath("Canvases/Board.canvas", "Canvases/Note.md")).toBe("Note.md");
    expect(canvasRelativePath("Board.canvas", "Product/Vision.md")).toBe("Product/Vision.md");
  });

  it("rejects absolute and escaping workspace paths", () => {
    expect(() => canvasRelativePath("Canvases/Board.canvas", "../outside.md")).toThrow();
    expect(() => canvasRelativePath("/Board.canvas", "Notes/A.md")).toThrow();
  });

  it("provides lightweight previews without mutating the source snapshot", () => {
    expect(previewMoveNodes(data, [{ id: "a", x: 10, y: 20 }]).nodes[0]).toMatchObject({ x: 10, y: 20 });
    expect(data.nodes[0]).toMatchObject({ x: 1, y: 2 });
    expect(previewRemoveNodes(data, ["a"]).edges).toHaveLength(0);
    expect(previewPlaceResource(data, "Product/C.md", { id: "c", x: 8, y: 9, width: 20, height: 30 }).nodes).toHaveLength(3);
    expect(previewAddEdge(data, { id: "ac", fromNode: "a", toNode: "b" }).edges).toHaveLength(2);
    expect(previewRemoveEdges(data, ["ab"]).edges).toHaveLength(0);
    expect(previewResizeNodes(data, [{ id: "a", width: 220, height: 160 }]).nodes[0]).toMatchObject({
      width: 220,
      height: 160,
    });
    expect(previewAddTextNode(data, {
      id: "note",
      text: "Sticky",
      x: 0,
      y: 0,
      width: 120,
      height: 80,
    }).nodes).toHaveLength(3);
    expect(previewUpdateTextNode(data, "a", "Updated").nodes[0]).toMatchObject({ text: "Updated" });
  });

  it("maps keyboard movement to bounded semantic deltas", () => {
    expect(keyboardMoveDelta("ArrowLeft")).toEqual({ x: -1, y: 0 });
    expect(keyboardMoveDelta("ArrowDown", true)).toEqual({ x: 0, y: 10 });
    expect(keyboardMoveDelta("Escape")).toBeNull();
    expect(canvasOutline(data)).toEqual([
      { id: "a", label: "Alpha" },
      { id: "b", label: "Notes/B.md" },
    ]);
  });
});
