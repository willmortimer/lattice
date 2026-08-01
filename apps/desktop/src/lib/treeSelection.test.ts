import { describe, expect, it } from "vitest";

import { nextTreeSelection, resourceIdsForTreeDrag } from "./treeSelection";

const visible = ["id-a", "id-b", "id-c", "id-d"] as const;

describe("nextTreeSelection", () => {
  it("replaces selection on plain click", () => {
    const result = nextTreeSelection({
      previous: new Set(["id-a", "id-b"]),
      anchor: "id-a",
      clicked: "id-c",
      visibleResourceIds: visible,
      mode: "replace",
    });
    expect([...result.selected]).toEqual(["id-c"]);
    expect(result.anchor).toBe("id-c");
  });

  it("toggles membership on toggle click", () => {
    const added = nextTreeSelection({
      previous: new Set(["id-a"]),
      anchor: "id-a",
      clicked: "id-c",
      visibleResourceIds: visible,
      mode: "toggle",
    });
    expect(added.selected).toEqual(new Set(["id-a", "id-c"]));
    expect(added.anchor).toBe("id-c");

    const removed = nextTreeSelection({
      previous: added.selected,
      anchor: "id-c",
      clicked: "id-a",
      visibleResourceIds: visible,
      mode: "toggle",
    });
    expect(removed.selected).toEqual(new Set(["id-c"]));
  });

  it("selects a contiguous visible range on shift-click", () => {
    const result = nextTreeSelection({
      previous: new Set(["id-a"]),
      anchor: "id-a",
      clicked: "id-c",
      visibleResourceIds: visible,
      mode: "range",
    });
    expect(result.selected).toEqual(new Set(["id-a", "id-b", "id-c"]));
    expect(result.anchor).toBe("id-a");
  });

  it("falls back to the clicked id when the anchor is not visible", () => {
    const result = nextTreeSelection({
      previous: new Set(),
      anchor: "id-hidden",
      clicked: "id-b",
      visibleResourceIds: visible,
      mode: "range",
    });
    expect(result.selected).toEqual(new Set(["id-b"]));
  });
});

describe("resourceIdsForTreeDrag", () => {
  it("moves the whole selection when the drag source is selected", () => {
    expect(resourceIdsForTreeDrag("id-b", new Set(["id-a", "id-b", "id-c"]))).toEqual([
      "id-a",
      "id-b",
      "id-c",
    ]);
  });

  it("moves only the drag source when it is outside the selection", () => {
    expect(resourceIdsForTreeDrag("id-d", new Set(["id-a", "id-b"]))).toEqual(["id-d"]);
  });
});
