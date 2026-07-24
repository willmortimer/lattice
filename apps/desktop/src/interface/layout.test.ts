import { describe, expect, it } from "vitest";

import type { InterfaceComponent } from "../lib/bindingSpec";
import {
  allocateComponentId,
  clampSpan,
  createDefaultComponent,
  insertComponent,
  layoutColumns,
  removeComponent,
  reorderComponents,
  resizeComponentSpan,
  updateComponent,
} from "./layout";

const sample: InterfaceComponent[] = [
  { id: "a", type: "metric", span: 3 },
  { id: "b", type: "chart", span: 6 },
  { id: "c", type: "map", span: 6 },
];

describe("interface layout helpers", () => {
  it("clamps span into the grid", () => {
    expect(clampSpan(0, 12)).toBe(1);
    expect(clampSpan(99, 12)).toBe(12);
    expect(clampSpan(4.7, 12)).toBe(4);
  });

  it("reorders by id", () => {
    expect(reorderComponents(sample, "a", "c").map((item) => item.id)).toEqual([
      "b",
      "c",
      "a",
    ]);
    expect(reorderComponents(sample, "a", "a")).toEqual(sample);
  });

  it("resizes a component span", () => {
    expect(resizeComponentSpan(sample, "b", 4)[1]?.span).toBe(4);
  });

  it("reads layout columns with default", () => {
    expect(layoutColumns(undefined)).toBe(12);
    expect(layoutColumns({ columns: 8 })).toBe(8);
  });

  it("inserts after a selected id and allocates unique ids", () => {
    const created = createDefaultComponent("data-view", sample, {
      views: ["Board"],
    });
    expect(created.id).toBe("data_view");
    expect(created.binding).toEqual({
      type: "saved-view",
      resource: ".",
      view: "Board",
    });
    const next = insertComponent(sample, created, "a");
    expect(next.map((item) => item.id)).toEqual(["a", "data_view", "b", "c"]);
    expect(allocateComponentId(next, "data-view")).toBe("data_view_2");
  });

  it("removes and patches components without dropping siblings", () => {
    expect(removeComponent(sample, "b").map((item) => item.id)).toEqual(["a", "c"]);
    const patched = updateComponent(sample, "a", {
      title: "Contacts",
      binding: {
        type: "sqlite-query",
        resource: ".",
        sql: "SELECT 1",
        limit: 1,
      },
    });
    expect(patched[0]).toMatchObject({
      id: "a",
      title: "Contacts",
      binding: { type: "sqlite-query", sql: "SELECT 1" },
    });
    expect(patched[1]).toEqual(sample[1]);
  });
});
