import { describe, expect, it } from "vitest";

import { nextTreeSelection, pathsForTreeDrag } from "./treeSelection";

const visible = ["A.md", "B.md", "C.md", "D.md"] as const;

describe("nextTreeSelection", () => {
  it("replaces selection on plain click", () => {
    const result = nextTreeSelection({
      previous: new Set(["A.md", "B.md"]),
      anchor: "A.md",
      clicked: "C.md",
      visibleFilePaths: visible,
      mode: "replace",
    });
    expect([...result.selected]).toEqual(["C.md"]);
    expect(result.anchor).toBe("C.md");
  });

  it("toggles membership on toggle click", () => {
    const added = nextTreeSelection({
      previous: new Set(["A.md"]),
      anchor: "A.md",
      clicked: "C.md",
      visibleFilePaths: visible,
      mode: "toggle",
    });
    expect(added.selected).toEqual(new Set(["A.md", "C.md"]));
    expect(added.anchor).toBe("C.md");

    const removed = nextTreeSelection({
      previous: added.selected,
      anchor: "C.md",
      clicked: "A.md",
      visibleFilePaths: visible,
      mode: "toggle",
    });
    expect(removed.selected).toEqual(new Set(["C.md"]));
  });

  it("selects a contiguous visible range on shift-click", () => {
    const result = nextTreeSelection({
      previous: new Set(["A.md"]),
      anchor: "A.md",
      clicked: "C.md",
      visibleFilePaths: visible,
      mode: "range",
    });
    expect(result.selected).toEqual(new Set(["A.md", "B.md", "C.md"]));
    expect(result.anchor).toBe("A.md");
  });

  it("falls back to the clicked path when the anchor is not visible", () => {
    const result = nextTreeSelection({
      previous: new Set(),
      anchor: "Hidden.md",
      clicked: "B.md",
      visibleFilePaths: visible,
      mode: "range",
    });
    expect(result.selected).toEqual(new Set(["B.md"]));
  });
});

describe("pathsForTreeDrag", () => {
  it("moves the whole selection when the drag source is selected", () => {
    expect(pathsForTreeDrag("B.md", new Set(["A.md", "B.md", "C.md"]))).toEqual([
      "A.md",
      "B.md",
      "C.md",
    ]);
  });

  it("moves only the drag source when it is outside the selection", () => {
    expect(pathsForTreeDrag("D.md", new Set(["A.md", "B.md"]))).toEqual(["D.md"]);
  });
});
