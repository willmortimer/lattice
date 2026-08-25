import { describe, expect, it, vi } from "vitest";
import * as Y from "yjs";

import { invoke } from "@tauri-apps/api/core";
import { applyResourceUpdate } from "../../lib/resourceRuntime";
import { applyCanvasDataToYDoc, canvasDataFromYDoc } from "./canvasYDoc";
import { createCollabCanvasAdapter } from "./collabCanvasAdapter";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("../../lib/resourceRuntime", () => ({
  applyResourceUpdate: vi.fn(),
}));

describe("createCollabCanvasAdapter", () => {
  it("mutates Y.Doc maps and does not imply a new disk revision", async () => {
    const ydoc = new Y.Doc();
    applyCanvasDataToYDoc(ydoc, {
      nodes: [
        { id: "a", type: "text", text: "A", x: 0, y: 0, width: 100, height: 80 },
        { id: "b", type: "file", file: "B.md", x: 120, y: 0, width: 100, height: 80 },
      ],
      edges: [],
    });
    const onLocalChange = vi.fn();
    const adapter = createCollabCanvasAdapter({
      ydoc,
      canvasPath: "Boards/Map.canvas",
      getRevision: () => "rev-0",
      onLocalChange,
    });

    expect(await adapter.moveNodes([{ id: "a", x: 8, y: 9 }], "rev-0")).toBe("rev-0");
    expect(await adapter.addEdge({
      edgeId: "ab",
      fromNode: "a",
      toNode: "b",
      baseRevision: "rev-0",
    })).toBe("rev-0");
    expect(await adapter.placeResource({
      resourcePath: "Notes/C.md",
      nodeId: "c",
      x: 240,
      y: 0,
      width: 100,
      height: 80,
      baseRevision: "rev-0",
    })).toBe("rev-0");

    expect(onLocalChange).toHaveBeenCalledTimes(3);
    const next = canvasDataFromYDoc(ydoc);
    expect(next.nodes.find((node) => node.id === "a")).toMatchObject({ x: 8, y: 9 });
    expect(next.nodes.find((node) => node.id === "c")).toMatchObject({
      type: "file",
      file: "../Notes/C.md",
    });
    expect(next.edges).toEqual([{ id: "ab", fromNode: "a", toNode: "b" }]);

    const snapshot = await adapter.read();
    expect(snapshot.revision).toBe("rev-0");
    expect(JSON.parse(snapshot.content)).toEqual(next);

    expect(vi.mocked(invoke)).not.toHaveBeenCalled();
    expect(vi.mocked(applyResourceUpdate)).not.toHaveBeenCalled();
  });

  it("does not invoke apply_canvas_edit IPC or applyResourceUpdate on gestures", async () => {
    const ydoc = new Y.Doc();
    applyCanvasDataToYDoc(ydoc, {
      nodes: [
        { id: "a", type: "text", text: "A", x: 0, y: 0, width: 100, height: 80 },
        { id: "b", type: "file", file: "B.md", x: 120, y: 0, width: 100, height: 80 },
      ],
      edges: [{ id: "ab", fromNode: "a", toNode: "b" }],
    });
    const adapter = createCollabCanvasAdapter({
      ydoc,
      canvasPath: "Boards/Map.canvas",
      getRevision: () => "rev-0",
      onLocalChange: () => undefined,
    });

    await adapter.resizeNodes([{ id: "a", width: 140, height: 90 }], "rev-0");
    await adapter.updateTextNode("a", "Alpha", "rev-0");
    await adapter.addTextNode({
      nodeId: "note",
      text: "Sticky",
      x: 0,
      y: 120,
      width: 120,
      height: 80,
      baseRevision: "rev-0",
    });
    await adapter.removeEdges(["ab"], "rev-0");
    await adapter.removeNodes(["b"], "rev-0");

    const commands = vi.mocked(invoke).mock.calls.map((call) => String(call[0]));
    expect(commands).not.toContain("apply_canvas_edit");
    expect(commands.filter((name) => name.startsWith("canvas_"))).toEqual([]);
    expect(vi.mocked(invoke)).not.toHaveBeenCalled();
    expect(vi.mocked(applyResourceUpdate)).not.toHaveBeenCalled();
  });
});
