import { describe, expect, it } from "vitest";

import type { InterfaceComponent } from "../lib/bindingSpec";
import {
  clampSpan,
  layoutColumns,
  reorderComponents,
  resizeComponentSpan,
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
});
