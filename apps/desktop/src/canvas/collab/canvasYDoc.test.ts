import { describe, expect, it } from "vitest";
import * as Y from "yjs";

import type { CanvasData } from "../types";
import { parseCanvas } from "../types";
import {
  applyCanvasDataToYDoc,
  canvasDataFromYDoc,
  canvasYDocIsEmpty,
  observeCanvasYDoc,
  yDocAddEdge,
  yDocAddTextNode,
  yDocMoveNodes,
  yDocPlaceFileNode,
  yDocRemoveEdges,
  yDocRemoveNodes,
  yDocResizeNodes,
  yDocUpdateTextNode,
} from "./canvasYDoc";

const SAMPLE: CanvasData = {
  nodes: [
    {
      id: "note",
      type: "text",
      text: "Hello board",
      x: 10,
      y: 20,
      width: 200,
      height: 140,
      color: "1",
    },
    {
      id: "page",
      type: "file",
      file: "Notes/Home.md",
      subpath: "#intro",
      x: 240,
      y: 20,
      width: 180,
      height: 120,
    },
    {
      id: "docs",
      type: "link",
      url: "https://jsoncanvas.org",
      x: 10,
      y: 200,
      width: 160,
      height: 80,
      color: "#ff8800",
    },
    {
      id: "cluster",
      type: "group",
      label: "Cluster",
      x: 0,
      y: 0,
      width: 500,
      height: 360,
    },
  ],
  edges: [
    {
      id: "e1",
      fromNode: "note",
      toNode: "page",
      fromSide: "right",
      toSide: "left",
      label: "opens",
      color: "4",
    },
    {
      id: "e2",
      fromNode: "page",
      toNode: "docs",
    },
  ],
};

describe("canvasYDoc", () => {
  it("round-trips text, file, link, and group nodes plus edges", () => {
    const ydoc = new Y.Doc();
    applyCanvasDataToYDoc(ydoc, SAMPLE);
    const next = canvasDataFromYDoc(ydoc);
    expect(next).toEqual(SAMPLE);
    expect(parseCanvas(next)).toEqual(SAMPLE);
  });

  it("round-trips through another peer via Yjs updates", () => {
    const source = new Y.Doc();
    applyCanvasDataToYDoc(source, SAMPLE);
    const target = new Y.Doc();
    Y.applyUpdate(target, Y.encodeStateAsUpdate(source));
    expect(canvasDataFromYDoc(target)).toEqual(SAMPLE);
  });

  it("treats a fresh Y.Doc as empty until seeded", () => {
    const ydoc = new Y.Doc();
    expect(canvasYDocIsEmpty(ydoc)).toBe(true);
    expect(canvasDataFromYDoc(ydoc)).toEqual({ nodes: [], edges: [] });
    applyCanvasDataToYDoc(ydoc, SAMPLE);
    expect(canvasYDocIsEmpty(ydoc)).toBe(false);
  });

  it("replaces a previous snapshot instead of merging stale ids", () => {
    const ydoc = new Y.Doc();
    applyCanvasDataToYDoc(ydoc, SAMPLE);
    applyCanvasDataToYDoc(ydoc, {
      nodes: [{ id: "only", type: "text", text: "Solo", x: 0, y: 0, width: 100, height: 80 }],
      edges: [],
    });
    expect(canvasDataFromYDoc(ydoc)).toEqual({
      nodes: [{ id: "only", type: "text", text: "Solo", x: 0, y: 0, width: 100, height: 80 }],
      edges: [],
    });
  });

  it("applies structural edits without rewriting the whole document", () => {
    const ydoc = new Y.Doc();
    applyCanvasDataToYDoc(ydoc, {
      nodes: [
        { id: "a", type: "text", text: "A", x: 0, y: 0, width: 100, height: 80 },
        { id: "b", type: "file", file: "B.md", x: 120, y: 0, width: 100, height: 80 },
      ],
      edges: [{ id: "ab", fromNode: "a", toNode: "b" }],
    });

    yDocMoveNodes(ydoc, [{ id: "a", x: 15, y: 25 }]);
    yDocResizeNodes(ydoc, [{ id: "b", width: 140, height: 90 }]);
    yDocUpdateTextNode(ydoc, "a", "Alpha");
    yDocPlaceFileNode(ydoc, {
      id: "c",
      file: "C.md",
      x: 240,
      y: 0,
      width: 100,
      height: 80,
    });
    yDocAddTextNode(ydoc, {
      id: "note",
      text: "Sticky",
      x: 0,
      y: 120,
      width: 120,
      height: 80,
    });
    yDocAddEdge(ydoc, { id: "ac", fromNode: "a", toNode: "c", fromSide: "bottom", toSide: "top" });
    yDocRemoveEdges(ydoc, ["ab"]);

    const next = canvasDataFromYDoc(ydoc);
    expect(next.nodes.find((node) => node.id === "a")).toMatchObject({ x: 15, y: 25, text: "Alpha" });
    expect(next.nodes.find((node) => node.id === "b")).toMatchObject({ width: 140, height: 90 });
    expect(next.nodes.map((node) => node.id)).toEqual(["a", "b", "c", "note"]);
    expect(next.edges).toEqual([
      { id: "ac", fromNode: "a", toNode: "c", fromSide: "bottom", toSide: "top" },
    ]);

    yDocRemoveNodes(ydoc, ["c"]);
    const afterRemove = canvasDataFromYDoc(ydoc);
    expect(afterRemove.nodes.map((node) => node.id)).toEqual(["a", "b", "note"]);
    expect(afterRemove.edges).toEqual([]);
  });

  it("drops edges whose endpoints disappeared from the maps", () => {
    const ydoc = new Y.Doc();
    applyCanvasDataToYDoc(ydoc, SAMPLE);
    yDocRemoveNodes(ydoc, ["note"]);
    const next = canvasDataFromYDoc(ydoc);
    expect(next.edges.every((edge) => edge.fromNode !== "note" && edge.toNode !== "note")).toBe(true);
  });

  it("observeCanvasYDoc emits on local edits and remote Yjs updates", () => {
    const local = new Y.Doc();
    applyCanvasDataToYDoc(local, {
      nodes: [
        { id: "a", type: "text", text: "A", x: 0, y: 0, width: 100, height: 80 },
        { id: "b", type: "file", file: "B.md", x: 120, y: 0, width: 100, height: 80 },
      ],
      edges: [],
    });

    const snapshots: ReturnType<typeof canvasDataFromYDoc>[] = [];
    const applyLive = () => {
      snapshots.push(canvasDataFromYDoc(local));
    };
    applyLive();
    const stop = observeCanvasYDoc(local, applyLive);

    yDocMoveNodes(local, [{ id: "a", x: 40, y: 50 }]);
    expect(snapshots.at(-1)?.nodes.find((node) => node.id === "a")).toMatchObject({ x: 40, y: 50 });

    const peer = new Y.Doc();
    Y.applyUpdate(peer, Y.encodeStateAsUpdate(local));
    yDocAddTextNode(peer, {
      id: "note",
      text: "Remote sticky",
      x: 0,
      y: 120,
      width: 120,
      height: 80,
    });
    Y.applyUpdate(local, Y.encodeStateAsUpdate(peer));
    expect(snapshots.at(-1)?.nodes.map((node) => node.id)).toEqual(["a", "b", "note"]);
    expect(snapshots.at(-1)?.nodes.find((node) => node.id === "note")).toMatchObject({
      text: "Remote sticky",
    });

    stop();
  });
});
