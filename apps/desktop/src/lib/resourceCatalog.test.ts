import { describe, expect, it } from "vitest";

import {
  applyCatalogDelta,
  catalogEntriesFromResources,
  catalogMapFromResources,
  isSyntheticResourceId,
  parentPathOf,
  resourcesFromCatalog,
  syntheticResourceId,
  type CatalogEntry,
} from "./resourceCatalog";
import type { Resource } from "../types";

function entry(
  resourceId: string,
  path: string,
  kind: Resource["kind"] = "page",
): CatalogEntry {
  return {
    resourceId,
    path,
    kind,
    parentId: parentPathOf(path) ? `parent-of-${path}` : undefined,
    childCount: 0,
  };
}

function resource(path: string, kind: Resource["kind"] = "page"): Resource {
  return { path, kind };
}

describe("resourceCatalog", () => {
  it("parentPathOf resolves folder parents", () => {
    expect(parentPathOf("Notes/a.md")).toBe("Notes");
    expect(parentPathOf("a.md")).toBeUndefined();
  });

  it("catalogEntriesFromResources uses synthetic ids until registry deltas arrive", () => {
    const entries = catalogEntriesFromResources([
      resource("Notes/a.md"),
      resource("root.md"),
    ]);
    expect(entries).toHaveLength(2);
    expect(isSyntheticResourceId(entries[0]?.resourceId ?? "")).toBe(true);
    expect(entries.find((item) => item.path === "Notes/a.md")?.parentId).toBe(
      syntheticResourceId("Notes"),
    );
  });

  it("applyCatalogDelta upsert remove and replace", () => {
    const base = applyCatalogDelta(new Map(), {
      type: "replace",
      entries: [entry("a", "a.md"), entry("b", "b.md")],
    });
    expect(base.size).toBe(2);

    const upserted = applyCatalogDelta(base, {
      type: "upsert",
      entries: [entry("c", "c.md")],
    });
    expect(upserted.size).toBe(3);

    const removed = applyCatalogDelta(upserted, {
      type: "remove",
      resourceIds: ["b"],
    });
    expect(removed.size).toBe(2);
    expect(removed.has("b")).toBe(false);

    const reordered = applyCatalogDelta(removed, {
      type: "reorder",
      orderedIds: ["c", "a"],
    });
    expect(reordered).toEqual(removed);
  });

  it("upsert replaces synthetic placeholder ids for the same path", () => {
    const seeded = catalogMapFromResources([resource("note.md")]);
    expect(seeded.has(syntheticResourceId("note.md"))).toBe(true);

    const updated = applyCatalogDelta(seeded, {
      type: "upsert",
      entries: [entry("stable-id", "note.md", "file")],
    });
    expect(updated.has(syntheticResourceId("note.md"))).toBe(false);
    expect(updated.get("stable-id")?.kind).toBe("file");
    expect(resourcesFromCatalog(updated).map((item) => item.path)).toEqual(["note.md"]);
  });
});
