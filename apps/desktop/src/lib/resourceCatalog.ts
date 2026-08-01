/**
 * Workspace resource catalog projection helpers.
 *
 * Mirrors `lattice_handlers::catalog` (C0) for shell-side incremental updates
 * from `catalog-delta` Tauri events. The flat `Resource[]` on
 * `WorkspaceSnapshot` is derived from the id-keyed catalog map.
 */
import { invoke } from "./ipc";
import type { Resource, ResourceKind } from "../types";

/** Compact catalog metadata row keyed by stable resource identity. */
export interface CatalogEntry {
  resourceId: string;
  path: string;
  kind: ResourceKind;
  parentId?: string;
  childCount: number;
}

/** Incremental catalog mutation emitted as `catalog-delta`. */
export type CatalogDelta =
  | { type: "upsert"; entries: CatalogEntry[] }
  | { type: "remove"; resourceIds: string[] }
  | { type: "reorder"; parentId?: string; orderedIds: string[] }
  | { type: "replace"; entries: CatalogEntry[] };

/** Wire payload for the `catalog-delta` Tauri event. */
export interface CatalogDeltaEvent {
  workspaceRoot: string;
  delta: CatalogDelta;
}

/** One page of direct children under a catalog parent. */
export interface ListChildrenPage {
  children: CatalogEntry[];
  nextCursor?: string;
}

const SYNTHETIC_ID_PREFIX = "path:";

/** Build a placeholder id until the namespace registry assigns a stable one. */
export function syntheticResourceId(path: string): string {
  return `${SYNTHETIC_ID_PREFIX}${path}`;
}

export function isSyntheticResourceId(resourceId: string): boolean {
  return resourceId.startsWith(SYNTHETIC_ID_PREFIX);
}

/** LatticeFS ResourceId wire form is a UUID string. */
export function looksLikeLatticeResourceId(value: string): boolean {
  return /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(
    value,
  );
}

/** Connected-root virtual paths opened outside the workspace catalog. */
export function looksLikeConnectedRootPath(path: string): boolean {
  return /^(github|gitlab):\/\//i.test(path);
}

/**
 * Inspect/session display id: prefer a registry UUID when known.
 * Never invent a fake UUID — fall back to an honest `path:` placeholder.
 */
export function displayResourceIdForPath(
  path: string,
  registryResourceId?: string | null,
): { resourceId: string; isSynthetic: boolean } {
  if (registryResourceId && looksLikeLatticeResourceId(registryResourceId)) {
    return { resourceId: registryResourceId, isSynthetic: false };
  }
  return { resourceId: syntheticResourceId(path), isSynthetic: true };
}

/** Immediate parent path key, or `undefined` for workspace-root entries. */
export function parentPathOf(path: string): string | undefined {
  const trimmed = path.replace(/^\/+|\/+$/g, "");
  if (!trimmed) return undefined;
  const slash = trimmed.lastIndexOf("/");
  if (slash < 0) return undefined;
  const parent = trimmed.slice(0, slash);
  return parent.length > 0 ? parent : undefined;
}

function childCounts(resources: readonly Resource[]): Map<string, number> {
  const counts = new Map<string, number>();
  for (const resource of resources) {
    const parent = parentPathOf(resource.path);
    if (parent) counts.set(parent, (counts.get(parent) ?? 0) + 1);
  }
  return counts;
}

/** Build catalog entries from an initial workspace snapshot scan. */
export function catalogEntriesFromResources(resources: readonly Resource[]): CatalogEntry[] {
  const counts = childCounts(resources);
  return resources.map((resource) => {
    const parent = parentPathOf(resource.path);
    return {
      resourceId: syntheticResourceId(resource.path),
      path: resource.path,
      kind: resource.kind,
      parentId: parent ? syntheticResourceId(parent) : undefined,
      childCount: counts.get(resource.path) ?? 0,
    };
  });
}

export function resourceFromCatalogEntry(entry: CatalogEntry): Resource {
  return { path: entry.path, kind: entry.kind };
}

export function resourcesFromCatalog(catalog: ReadonlyMap<string, CatalogEntry>): Resource[] {
  return [...catalog.values()]
    .map(resourceFromCatalogEntry)
    .sort((left, right) => left.path.localeCompare(right.path));
}

export function catalogMapFromResources(resources: readonly Resource[]): Map<string, CatalogEntry> {
  const entries = catalogEntriesFromResources(resources);
  return applyCatalogDelta(new Map(), { type: "replace", entries });
}

/** Reverse index: workspace-relative path → catalog resourceId. */
export function catalogPathToIdMap(
  catalog: ReadonlyMap<string, CatalogEntry>,
): Map<string, string> {
  const byPath = new Map<string, string>();
  for (const entry of catalog.values()) {
    byPath.set(entry.path, entry.resourceId);
  }
  return byPath;
}

/** Resolve a path alias to its catalog ResourceId when present. */
export function resourceIdForPath(
  catalog: ReadonlyMap<string, CatalogEntry>,
  path: string,
): string | undefined {
  for (const entry of catalog.values()) {
    if (entry.path === path) return entry.resourceId;
  }
  return undefined;
}

/** Resolve a ResourceId to its current path alias (survives renames). */
export function pathForResourceId(
  catalog: ReadonlyMap<string, CatalogEntry>,
  resourceId: string,
): string | undefined {
  return catalog.get(resourceId)?.path;
}

/**
 * Persistable id for a path: prefer registry/catalog UUID, else synthetic
 * `path:` placeholder until a stable id arrives.
 */
export function resourceIdForPathOrSynthetic(
  catalog: ReadonlyMap<string, CatalogEntry>,
  path: string,
): string {
  return resourceIdForPath(catalog, path) ?? syntheticResourceId(path);
}

/** Resolve selection ids to current workspace paths (skips missing ids). */
export function pathsForResourceIds(
  catalog: ReadonlyMap<string, CatalogEntry>,
  resourceIds: ReadonlySet<string> | readonly string[],
): string[] {
  const paths: string[] = [];
  for (const resourceId of resourceIds) {
    const path = pathForResourceId(catalog, resourceId);
    if (path) paths.push(path);
  }
  return paths;
}

/**
 * Keep tree/tab selection identity across catalog deltas: preserve ids that
 * still exist, and remap synthetic→registry (same path, new id). Dropped
 * entries are removed from the set.
 *
 * Honest `path:` placeholders for connected-root virtual paths (never present
 * in the workspace catalog) are retained so selection does not invent UUIDs.
 */
export function remapSelectedResourceIds(
  selected: ReadonlySet<string>,
  previous: ReadonlyMap<string, CatalogEntry>,
  next: ReadonlyMap<string, CatalogEntry>,
): Set<string> {
  const remapped = new Set<string>();
  for (const id of selected) {
    if (next.has(id)) {
      remapped.add(id);
      continue;
    }
    const previousEntry = previous.get(id);
    if (!previousEntry) {
      if (isSyntheticResourceId(id)) {
        const path = id.slice(SYNTHETIC_ID_PREFIX.length);
        const replacement = resourceIdForPath(next, path);
        if (replacement) {
          remapped.add(replacement);
        } else if (looksLikeConnectedRootPath(path)) {
          // Connected-root virtual paths are outside the workspace catalog.
          remapped.add(id);
        }
      }
      continue;
    }
    const replacement = resourceIdForPath(next, previousEntry.path);
    if (replacement) remapped.add(replacement);
  }
  return remapped;
}

/** Apply a catalog delta to an id-keyed catalog map. */
export function applyCatalogDelta(
  current: ReadonlyMap<string, CatalogEntry>,
  delta: CatalogDelta,
): Map<string, CatalogEntry> {
  switch (delta.type) {
    case "replace": {
      const next = new Map<string, CatalogEntry>();
      for (const entry of delta.entries) {
        next.set(entry.resourceId, entry);
      }
      return next;
    }
    case "upsert": {
      const next = new Map(current);
      for (const entry of delta.entries) {
        for (const [existingId, existing] of next) {
          if (existing.path === entry.path && existingId !== entry.resourceId) {
            next.delete(existingId);
          }
        }
        next.set(entry.resourceId, entry);
      }
      return next;
    }
    case "remove": {
      const next = new Map(current);
      for (const resourceId of delta.resourceIds) {
        next.delete(resourceId);
      }
      return next;
    }
    case "reorder":
      // Order is carried on the delta for future tree consumers; map is unchanged.
      return new Map(current);
    default:
      return delta satisfies never;
  }
}

/** Paginated direct children for a catalog parent (stable id and/or path). */
export async function listChildren(args: {
  root: string;
  parentId?: string;
  parentPath?: string;
  cursor?: string;
  limit?: number;
}): Promise<ListChildrenPage> {
  return invoke<ListChildrenPage>("list_children", args);
}
