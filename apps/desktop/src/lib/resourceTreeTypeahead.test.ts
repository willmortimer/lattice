import { describe, expect, it } from "vitest";

import type { FlatRow } from "./resourceTree";
import {
  appendTreeTypeaheadPrefix,
  findNextTypeaheadRowIndex,
  isTreeTypeaheadKey,
  rowMatchesTypeaheadPrefix,
} from "./resourceTreeTypeahead";

function row(
  type: FlatRow["type"],
  name: string,
  overrides: Partial<FlatRow> = {},
): FlatRow {
  const base = {
    depth: 0,
    path: name,
    resourceId: `id:${name}`,
    name,
  };
  if (type === "file") {
    return {
      ...base,
      type: "file",
      resource: { path: name, kind: "page" },
      ...overrides,
    } as FlatRow;
  }
  if (type === "folder") {
    return {
      ...base,
      type: "folder",
      folder: {
        type: "folder",
        name,
        path: name,
        resourceId: `id:${name}`,
        children: [],
      },
      ...overrides,
    } as FlatRow;
  }
  return {
    ...base,
    type: "empty-folder",
    folder: {
      type: "folder",
      name,
      path: name,
      resourceId: `id:${name}`,
      children: [],
    },
    ...overrides,
  } as FlatRow;
}

describe("resourceTreeTypeahead", () => {
  it("accepts printable unicode letters and digits", () => {
    expect(isTreeTypeaheadKey("a")).toBe(true);
    expect(isTreeTypeaheadKey("É")).toBe(true);
    expect(isTreeTypeaheadKey("1")).toBe(true);
    expect(isTreeTypeaheadKey("Enter")).toBe(false);
    expect(isTreeTypeaheadKey(" ")).toBe(false);
  });

  it("matches row names case-insensitively from the start", () => {
    expect(rowMatchesTypeaheadPrefix("Apple.md", "ap")).toBe(true);
    expect(rowMatchesTypeaheadPrefix("banana.md", "Ap")).toBe(false);
  });

  it("appends typed characters to the active prefix", () => {
    expect(appendTreeTypeaheadPrefix("no", "t")).toBe("not");
  });

  it("finds the next matching visible row and wraps", () => {
    const rows: FlatRow[] = [
      row("folder", "Notes"),
      row("file", "Alpha.md"),
      row("file", "Beta.md"),
      row("empty-folder", "Inbox"),
    ];

    expect(findNextTypeaheadRowIndex(rows, "be", 1)).toBe(2);
    expect(findNextTypeaheadRowIndex(rows, "no", -1)).toBe(0);
    expect(findNextTypeaheadRowIndex(rows, "z", 0)).toBeNull();
    expect(findNextTypeaheadRowIndex(rows, "a", 2)).toBe(1);
  });
});
