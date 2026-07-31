/**
 * Workspace resource catalog projection helpers.
 *
 * The shell still receives a full WorkspaceSnapshot today. These helpers are
 * the seam for incremental catalog deltas so open time can become independent
 * of total resource count without rewriting every consumer at once.
 */
import type { Resource } from "../types";

export type ResourceCatalogDelta =
  | { type: "upsert"; resources: readonly Resource[] }
  | { type: "remove"; paths: readonly string[] }
  | { type: "replace"; resources: readonly Resource[] };

/** Apply a catalog delta to a path-keyed resource map. */
export function applyResourceCatalogDelta(
  current: ReadonlyMap<string, Resource>,
  delta: ResourceCatalogDelta,
): Map<string, Resource> {
  switch (delta.type) {
    case "replace": {
      const next = new Map<string, Resource>();
      for (const resource of delta.resources) {
        next.set(resource.path, resource);
      }
      return next;
    }
    case "upsert": {
      const next = new Map(current);
      for (const resource of delta.resources) {
        next.set(resource.path, resource);
      }
      return next;
    }
    case "remove": {
      const next = new Map(current);
      for (const path of delta.paths) {
        next.delete(path);
      }
      return next;
    }
    default:
      return delta satisfies never;
  }
}

export function resourcesFromCatalog(catalog: ReadonlyMap<string, Resource>): Resource[] {
  return [...catalog.values()].sort((a, b) => a.path.localeCompare(b.path));
}

export function catalogFromResources(resources: readonly Resource[]): Map<string, Resource> {
  const catalog = new Map<string, Resource>();
  for (const resource of resources) {
    catalog.set(resource.path, resource);
  }
  return catalog;
}
