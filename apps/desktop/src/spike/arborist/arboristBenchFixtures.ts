/**
 * Synthetic catalog fixtures for arborist scale probes (10k / 100k leaves).
 *
 * Shapes a wide fan-out: `Scale/{i}/leaf-{j}.md` with stable synthetic ids.
 */
import {
  applyCatalogDelta,
  syntheticResourceId,
  type CatalogEntry,
} from "../../lib/resourceCatalog";
import type { ResourceKind } from "../../types";

function leafEntry(folderIndex: number, leafIndex: number): CatalogEntry {
  const folderPath = `Scale/${folderIndex}`;
  const path = `${folderPath}/leaf-${leafIndex}.md`;
  return {
    resourceId: syntheticResourceId(path),
    path,
    kind: "page" satisfies ResourceKind,
    parentId: syntheticResourceId(folderPath),
    childCount: 0,
  };
}

function folderEntry(folderIndex: number): CatalogEntry {
  const path = `Scale/${folderIndex}`;
  return {
    resourceId: syntheticResourceId(path),
    path,
    kind: "folder",
    parentId: syntheticResourceId("Scale"),
    childCount: 0,
  };
}

function scaleFolderEntry(): CatalogEntry {
  return {
    resourceId: syntheticResourceId("Scale"),
    path: "Scale",
    kind: "folder",
    parentId: undefined,
    childCount: 0,
  };
}

/** Build a flat catalog map with `leafCount` page resources under `Scale/{i}/`. */
export function syntheticCatalogForLeafCount(leafCount: number): Map<string, CatalogEntry> {
  if (leafCount <= 0) return new Map();

  const folders = Math.max(1, Math.ceil(Math.sqrt(leafCount)));
  const entries: CatalogEntry[] = [scaleFolderEntry()];

  for (let folderIndex = 0; folderIndex < folders; folderIndex++) {
    entries.push(folderEntry(folderIndex));
  }

  for (let index = 0; index < leafCount; index++) {
    const folderIndex = index % folders;
    entries.push(leafEntry(folderIndex, index));
  }

  return applyCatalogDelta(new Map(), { type: "replace", entries });
}
