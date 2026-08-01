/**
 * Catalog → react-arborist forest projection (C1 resourceId keys).
 *
 * Spike-only: not wired into production `ResourceTree`. Keeps stable
 * `resourceId` as the arborist node identity via `idAccessor`.
 */
import type { CatalogEntry } from "../../lib/resourceCatalog";
import { parentPathOf, syntheticResourceId } from "../../lib/resourceCatalog";
import type { ResourceKind } from "../../types";

/** One row in the arborist-controlled forest. */
export interface ArboristCatalogNode {
  resourceId: string;
  path: string;
  name: string;
  kind: ResourceKind;
  isFolder: boolean;
  children: ArboristCatalogNode[];
}

function leafName(path: string): string {
  const segments = path.split("/").filter((segment) => segment.length > 0);
  return segments[segments.length - 1] ?? path;
}

function sortSiblings(nodes: ArboristCatalogNode[]): void {
  nodes.sort((left, right) => {
    if (left.isFolder !== right.isFolder) return left.isFolder ? -1 : 1;
    return left.name.localeCompare(right.name, undefined, { sensitivity: "base" });
  });
  for (const node of nodes) {
    if (node.children.length > 0) sortSiblings(node.children);
  }
}

function folderNode(path: string, resourceId: string): ArboristCatalogNode {
  return {
    resourceId,
    path,
    name: leafName(path),
    kind: "folder",
    isFolder: true,
    children: [],
  };
}

function fileNode(entry: CatalogEntry): ArboristCatalogNode {
  return {
    resourceId: entry.resourceId,
    path: entry.path,
    name: leafName(entry.path),
    kind: entry.kind,
    isFolder: entry.kind === "folder",
    children: [],
  };
}

function parentIdFromPath(path: string): string | undefined {
  const parentPath = parentPathOf(path);
  return parentPath ? syntheticResourceId(parentPath) : undefined;
}

function ensureFolderNode(nodes: Map<string, ArboristCatalogNode>, resourceId: string): void {
  if (nodes.has(resourceId)) return;
  const path = resourceId.startsWith("path:") ? resourceId.slice("path:".length) : resourceId;
  nodes.set(resourceId, folderNode(path, resourceId));
}

/**
 * Build a react-arborist forest from the flat C1 catalog map.
 *
 * Missing intermediate folders are synthesized with `syntheticResourceId(path)`
 * so parent chains stay stable before registry ids arrive.
 */
export function catalogToArboristForest(
  catalog: ReadonlyMap<string, CatalogEntry>,
): ArboristCatalogNode[] {
  const nodes = new Map<string, ArboristCatalogNode>();
  const childrenByParent = new Map<string | null, string[]>();

  for (const entry of catalog.values()) {
    nodes.set(entry.resourceId, fileNode(entry));
    const parentId = entry.parentId ?? parentIdFromPath(entry.path) ?? null;
    const siblings = childrenByParent.get(parentId);
    if (siblings) siblings.push(entry.resourceId);
    else childrenByParent.set(parentId, [entry.resourceId]);
  }

  for (const [parentId, childIds] of childrenByParent) {
    if (parentId !== null) ensureFolderNode(nodes, parentId);
    const parent = parentId === null ? null : nodes.get(parentId);
    if (parentId !== null && !parent) continue;
    for (const childId of childIds) {
      const child = nodes.get(childId);
      if (!child) continue;
      if (parent) parent.children.push(child);
    }
  }

  const roots = (childrenByParent.get(null) ?? [])
    .map((resourceId) => nodes.get(resourceId))
    .filter((node): node is ArboristCatalogNode => node !== undefined);

  sortSiblings(roots);
  return roots;
}

/** Resolve workspace paths for semantic `move_resources` from arborist drag ids. */
export function pathsForArboristMove(
  catalog: ReadonlyMap<string, CatalogEntry>,
  dragResourceIds: readonly string[],
  parentResourceId: string | null,
): { fromPaths: string[]; toDir: string } | null {
  const fromPaths: string[] = [];
  for (const resourceId of dragResourceIds) {
    const entry = catalog.get(resourceId);
    if (!entry || entry.kind === "folder") return null;
    fromPaths.push(entry.path);
  }
  if (fromPaths.length === 0) return null;

  if (parentResourceId === null) {
    return { fromPaths, toDir: "" };
  }

  const parent = catalog.get(parentResourceId);
  const parentPath =
    parent?.path ??
    (parentResourceId.startsWith("path:")
      ? parentResourceId.slice("path:".length)
      : undefined);
  if (parentPath === undefined) return null;

  return { fromPaths, toDir: parentPath };
}

/** Default arborist search: case-insensitive match on leaf name or full path. */
export function arboristCatalogSearchMatch(
  node: { data: ArboristCatalogNode },
  searchTerm: string,
): boolean {
  const needle = searchTerm.trim().toLowerCase();
  if (!needle) return true;
  const { name, path } = node.data;
  return name.toLowerCase().includes(needle) || path.toLowerCase().includes(needle);
}
