import { describe, expect, it } from "vitest";

import {
  applyCatalogDelta,
  catalogEntriesFromResources,
  catalogFromOpenSnapshot,
  catalogMapFromResources,
  displayResourceIdForPath,
  isSyntheticResourceId,
  parentPathOf,
  pathForResourceId,
  pathsForResourceIds,
  remapSelectedResourceIds,
  resourceIdForPath,
  resourceIdForPathOrSynthetic,
  resourcesFromCatalog,
  seedCatalogFromListChildrenPage,
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

  it("resourceIdForPath and pathForResourceId round-trip", () => {
    const catalog = applyCatalogDelta(new Map(), {
      type: "replace",
      entries: [entry("uuid-1", "Notes/a.md")],
    });
    expect(resourceIdForPath(catalog, "Notes/a.md")).toBe("uuid-1");
    expect(pathForResourceId(catalog, "uuid-1")).toBe("Notes/a.md");
    expect(resourceIdForPathOrSynthetic(catalog, "Notes/a.md")).toBe("uuid-1");
    expect(resourceIdForPathOrSynthetic(catalog, "missing.md")).toBe(
      syntheticResourceId("missing.md"),
    );
    expect(pathsForResourceIds(catalog, new Set(["uuid-1", "missing"]))).toEqual([
      "Notes/a.md",
    ]);
  });

  it("remapSelectedResourceIds keeps UUID selection across path rename", () => {
    const before = applyCatalogDelta(new Map(), {
      type: "replace",
      entries: [entry("uuid-1", "old.md")],
    });
    const after = applyCatalogDelta(before, {
      type: "upsert",
      entries: [entry("uuid-1", "new.md")],
    });
    expect(remapSelectedResourceIds(new Set(["uuid-1"]), before, after)).toEqual(
      new Set(["uuid-1"]),
    );
  });

  it("remapSelectedResourceIds migrates synthetic selection onto registry UUID", () => {
    const before = catalogMapFromResources([resource("note.md")]);
    const synthetic = syntheticResourceId("note.md");
    const after = applyCatalogDelta(before, {
      type: "upsert",
      entries: [entry("stable-id", "note.md")],
    });
    expect(remapSelectedResourceIds(new Set([synthetic]), before, after)).toEqual(
      new Set(["stable-id"]),
    );
  });

  it("remapSelectedResourceIds keeps connected-root synthetics outside the catalog", () => {
    const before = catalogMapFromResources([resource("note.md")]);
    const connected = syntheticResourceId("github://acme/demo/README.md");
    const after = applyCatalogDelta(before, {
      type: "upsert",
      entries: [entry("stable-id", "note.md")],
    });
    expect(
      remapSelectedResourceIds(new Set([connected, syntheticResourceId("note.md")]), before, after),
    ).toEqual(new Set([connected, "stable-id"]));
  });

  it("displayResourceIdForPath never invents a UUID", () => {
    expect(displayResourceIdForPath("Notes.md", null)).toEqual({
      resourceId: syntheticResourceId("Notes.md"),
      isSynthetic: true,
    });
    expect(
      displayResourceIdForPath("Notes.md", "11111111-1111-1111-1111-111111111111"),
    ).toEqual({
      resourceId: "11111111-1111-1111-1111-111111111111",
      isSynthetic: false,
    });
    expect(displayResourceIdForPath("Notes.md", "not-a-uuid")).toEqual({
      resourceId: syntheticResourceId("Notes.md"),
      isSynthetic: true,
    });
  });

  it("catalogFromOpenSnapshot does not require a snapshot.resources dump", () => {
    expect(catalogFromOpenSnapshot([]).size).toBe(0);
    expect(catalogFromOpenSnapshot(undefined).size).toBe(0);

    const seeded = catalogFromOpenSnapshot([resource("root.md")]);
    expect(seeded.size).toBe(1);
    expect(resourceIdForPath(seeded, "root.md")).toBe(syntheticResourceId("root.md"));
  });

  it("catalog seeds from list_children pages and catalog-delta without a full dump", () => {
    let catalog = catalogFromOpenSnapshot([]);
    expect(catalog.size).toBe(0);

    catalog = seedCatalogFromListChildrenPage(catalog, {
      children: [entry("notes-id", "Notes", "folder"), entry("root-id", "root.md")],
    });
    expect(catalog.size).toBe(2);
    expect(catalog.get("notes-id")?.kind).toBe("folder");
    expect(catalog.get("root-id")?.path).toBe("root.md");

    catalog = seedCatalogFromListChildrenPage(catalog, {
      children: [entry("nested-id", "Notes/a.md")],
    });
    expect(catalog.size).toBe(3);
    expect(pathForResourceId(catalog, "nested-id")).toBe("Notes/a.md");

    catalog = applyCatalogDelta(catalog, {
      type: "upsert",
      entries: [entry("nested-id", "Notes/renamed.md")],
    });
    expect(pathForResourceId(catalog, "nested-id")).toBe("Notes/renamed.md");
    expect(resourcesFromCatalog(catalog).map((item) => item.path)).toEqual([
      "Notes",
      "Notes/renamed.md",
      "root.md",
    ]);
  });
});
