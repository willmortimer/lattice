import { describe, expect, it } from "vitest";

import {
  MAX_OVERLAY_ANCHORS,
  parseWorkspaceAnchor,
  serializeWorkspaceAnchor,
  workspaceAnchorSchema,
  type WorkspaceAnchor,
} from "./anchors";

describe("workspace anchors", () => {
  const validFixtures: WorkspaceAnchor[] = [
    {
      kind: "markdown-block",
      resourceId: "page:notes",
      blockId: "blk-1",
    },
    {
      kind: "markdown-block",
      resourceId: "page:notes",
      revision: "rev-3",
      blockId: "blk-heading",
    },
    {
      kind: "dataset-region",
      resourceId: "table:inventory",
      rowKeys: ["row-7"],
    },
    {
      kind: "dataset-region",
      resourceId: "table:inventory",
      revision: "rev-9",
      rowKeys: ["row-1", "row-2"],
      columns: ["sku", "qty"],
    },
  ];

  it.each(validFixtures)("accepts valid anchor %o", (anchor) => {
    expect(workspaceAnchorSchema.parse(anchor)).toEqual(anchor);
    const line = serializeWorkspaceAnchor(anchor);
    expect(parseWorkspaceAnchor(JSON.parse(line))).toEqual(anchor);
  });

  it("rejects unknown anchor kinds", () => {
    expect(() =>
      workspaceAnchorSchema.parse({
        kind: "canvas-node",
        resourceId: "canvas:1",
        nodeIds: ["n1"],
      }),
    ).toThrow();
  });

  it("rejects empty resourceId, blockId, and rowKeys", () => {
    expect(() =>
      workspaceAnchorSchema.parse({
        kind: "markdown-block",
        resourceId: "",
        blockId: "blk-1",
      }),
    ).toThrow();
    expect(() =>
      workspaceAnchorSchema.parse({
        kind: "markdown-block",
        resourceId: "page:notes",
        blockId: "",
      }),
    ).toThrow();
    expect(() =>
      workspaceAnchorSchema.parse({
        kind: "dataset-region",
        resourceId: "table:inventory",
        rowKeys: [],
      }),
    ).toThrow();
    expect(() =>
      workspaceAnchorSchema.parse({
        kind: "dataset-region",
        resourceId: "table:inventory",
        rowKeys: [""],
      }),
    ).toThrow();
  });

  it("rejects missing required fields", () => {
    expect(() =>
      workspaceAnchorSchema.parse({
        kind: "markdown-block",
        resourceId: "page:notes",
      }),
    ).toThrow();
    expect(() =>
      workspaceAnchorSchema.parse({
        kind: "dataset-region",
        resourceId: "table:inventory",
      }),
    ).toThrow();
  });

  it("exports the Phase C overlay anchor cap", () => {
    expect(MAX_OVERLAY_ANCHORS).toBe(20);
  });
});
