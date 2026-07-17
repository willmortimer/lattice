import { describe, expect, it } from "vitest";
import { parseStructuredSource } from "./structuredParserCore";
import { defaultExpandedIds, flattenVisibleTree } from "./structuredTreeModel";

describe("structuredTreeModel", () => {
  it("expands only the first two levels by default", () => {
    const parsed = parseStructuredSource('{"level1":{"level2":{"level3":1}}}', "json");
    expect(parsed.ok).toBe(true);
    if (!parsed.ok) return;
    const expanded = defaultExpandedIds(parsed.root);
    expect(expanded.has("root")).toBe(true);
    expect(expanded.has("root.level1")).toBe(true);
    expect(expanded.has("root.level1.level2")).toBe(true);
    expect(expanded.has("root.level1.level2.level3")).toBe(false);
  });

  it("flattens only visible rows for expand-on-demand rendering", () => {
    const parsed = parseStructuredSource('{"a":{"b":1},"c":2}', "json");
    expect(parsed.ok).toBe(true);
    if (!parsed.ok) return;
    const expanded = new Set(["root", "root.a"]);
    const rows = flattenVisibleTree(parsed.root, expanded);
    expect(rows.map((row) => row.label)).toEqual([
      "value: {2}",
      "a: {1}",
      "b: 1",
      "c: 2",
    ]);
  });
});
