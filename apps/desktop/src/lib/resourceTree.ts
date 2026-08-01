import type { Resource } from "../types";
import {
  parentPathOf,
  resourceFromCatalogEntry,
  syntheticResourceId,
  type CatalogDelta,
  type CatalogEntry,
} from "./resourceCatalog";

/** A folder grouping other nodes, keyed by its full path from the workspace root. */
export interface TreeFolder {
  type: "folder";
  name: string;
  path: string;
  resourceId: string;
  children: TreeNode[];
}

/** A leaf node wrapping one resource. */
export interface TreeFile {
  type: "file";
  name: string;
  resourceId: string;
  resource: Resource;
}

export type TreeNode = TreeFolder | TreeFile;

/** Fixed row height for the virtualized sidebar tree (must match CSS). */
export const RESOURCE_TREE_ROW_HEIGHT = 30;

export type FlatRow =
  | {
      type: "folder";
      depth: number;
      path: string;
      resourceId: string;
      name: string;
      folder: TreeFolder;
    }
  | {
      type: "file";
      depth: number;
      path: string;
      resourceId: string;
      name: string;
      resource: Resource;
    }
  | {
      type: "empty-folder";
      depth: number;
      path: string;
      resourceId: string;
      name: string;
      folder: TreeFolder;
    };

function leafName(path: string): string {
  const segments = path.split("/").filter((segment) => segment.length > 0);
  return segments[segments.length - 1] ?? path;
}

function parentIdForEntry(entry: CatalogEntry): string | undefined {
  if (entry.parentId) return entry.parentId;
  const parentPath = parentPathOf(entry.path);
  return parentPath ? syntheticResourceId(parentPath) : undefined;
}

/**
 * Depth-first list of visible tree rows, honoring collapsed folder paths.
 * Folder rows always precede their visible descendants; sibling order matches
 * `buildResourceTree` (folders before files, alphabetical within each group).
 */
export function flattenVisibleTree(
  nodes: readonly TreeNode[],
  collapsed: ReadonlySet<string>,
): FlatRow[] {
  const rows: FlatRow[] = [];

  function visit(nodeList: readonly TreeNode[], depth: number): void {
    for (const node of nodeList) {
      if (node.type === "file") {
        rows.push({
          type: "file",
          depth,
          path: node.resource.path,
          resourceId: node.resourceId,
          name: node.name,
          resource: node.resource,
        });
        continue;
      }

      rows.push({
        type: "folder",
        depth,
        path: node.path,
        resourceId: node.resourceId,
        name: node.name,
        folder: node,
      });

      if (collapsed.has(node.path)) continue;

      if (node.children.length === 0) {
        rows.push({
          type: "empty-folder",
          depth: depth + 1,
          path: node.path,
          resourceId: node.resourceId,
          name: node.name,
          folder: node,
        });
        continue;
      }

      visit(node.children, depth + 1);
    }
  }

  visit(nodes, 0);
  return rows;
}

/**
 * Build a collapsible folder tree from a flat resource listing (as
 * returned by `list_resources`), grouping by `/`-separated path segments.
 * Resources with kind `folder` ensure an empty folder node exists without
 * adding a file leaf. Within each folder, subfolders sort before files,
 * and both sort alphabetically (case-insensitive).
 *
 * Nodes carry synthetic `path:` resourceIds so selection can migrate onto
 * catalog UUIDs when registry deltas arrive.
 */
export function buildResourceTree(resources: readonly Resource[]): TreeNode[] {
  const root: TreeFolder = {
    type: "folder",
    name: "",
    path: "",
    resourceId: syntheticResourceId(""),
    children: [],
  };

  for (const resource of resources) {
    const segments = resource.path.split("/").filter((segment) => segment.length > 0);
    if (segments.length === 0) continue;

    if (resource.kind === "folder") {
      let cursor = root;
      for (let depth = 0; depth < segments.length; depth++) {
        const name = segments[depth];
        const path = segments.slice(0, depth + 1).join("/");
        let folder = cursor.children.find(
          (node): node is TreeFolder => node.type === "folder" && node.name === name,
        );
        if (!folder) {
          folder = {
            type: "folder",
            name,
            path,
            resourceId: syntheticResourceId(path),
            children: [],
          };
          cursor.children.push(folder);
        }
        cursor = folder;
      }
      continue;
    }

    let cursor = root;
    for (let depth = 0; depth < segments.length - 1; depth++) {
      const name = segments[depth];
      const path = segments.slice(0, depth + 1).join("/");
      let folder = cursor.children.find(
        (node): node is TreeFolder => node.type === "folder" && node.name === name,
      );
      if (!folder) {
        folder = {
          type: "folder",
          name,
          path,
          resourceId: syntheticResourceId(path),
          children: [],
        };
        cursor.children.push(folder);
      }
      cursor = folder;
    }

    const name = segments[segments.length - 1];
    cursor.children.push({
      type: "file",
      name,
      resourceId: syntheticResourceId(resource.path),
      resource,
    });
  }

  sortTree(root);
  return root.children;
}

/**
 * Build the sidebar forest from the id-keyed catalog map (parentId links).
 * Intermediate folders missing from the catalog are synthesized with
 * `path:` ids so the tree stays contiguous before registry ids arrive.
 */
export function buildResourceTreeFromCatalog(
  catalog: ReadonlyMap<string, CatalogEntry>,
): TreeNode[] {
  const nodes = new Map<string, TreeNode>();
  const childrenByParent = new Map<string | null, string[]>();

  function linkChild(parentId: string | null, childId: string): void {
    const siblings = childrenByParent.get(parentId);
    if (siblings) {
      if (!siblings.includes(childId)) siblings.push(childId);
    } else {
      childrenByParent.set(parentId, [childId]);
    }
  }

  function ensureFolderChain(resourceId: string): void {
    if (nodes.has(resourceId)) return;
    const path = resourceId.startsWith("path:") ? resourceId.slice("path:".length) : resourceId;
    if (!path) return;
    nodes.set(resourceId, {
      type: "folder",
      name: leafName(path),
      path,
      resourceId,
      children: [],
    });
    const parentPath = parentPathOf(path);
    const parentId = parentPath ? syntheticResourceId(parentPath) : null;
    linkChild(parentId, resourceId);
    if (parentId) ensureFolderChain(parentId);
  }

  for (const entry of catalog.values()) {
    nodes.set(entry.resourceId, nodeFromCatalogEntry(entry));
    const parentId = parentIdForEntry(entry) ?? null;
    if (parentId) ensureFolderChain(parentId);
    linkChild(parentId, entry.resourceId);
  }

  for (const [parentId, childIds] of childrenByParent) {
    if (parentId === null) continue;
    const parent = nodes.get(parentId);
    if (!parent || parent.type !== "folder") continue;
    // Rebuild children from the link index (idempotent if called once).
    parent.children = [];
    for (const childId of childIds) {
      const child = nodes.get(childId);
      if (child) parent.children.push(child);
    }
  }

  const roots = (childrenByParent.get(null) ?? [])
    .map((resourceId) => nodes.get(resourceId))
    .filter((node): node is TreeNode => node !== undefined);

  const rootFolder: TreeFolder = {
    type: "folder",
    name: "",
    path: "",
    resourceId: syntheticResourceId(""),
    children: roots,
  };
  sortTree(rootFolder);
  return rootFolder.children;
}

export interface ApplyCatalogDeltaToForestResult {
  forest: TreeNode[];
  /** True when the forest was rebuilt from the full catalog. */
  rebuilt: boolean;
}

/**
 * Patch a catalog-backed forest for upsert/remove deltas without a full
 * path-segment rebuild. Replace/reorder (and failed patches) fall back to
 * `buildResourceTreeFromCatalog`.
 */
export function applyCatalogDeltaToForest(
  forest: readonly TreeNode[],
  previousCatalog: ReadonlyMap<string, CatalogEntry>,
  delta: CatalogDelta,
  nextCatalog: ReadonlyMap<string, CatalogEntry>,
): ApplyCatalogDeltaToForestResult {
  switch (delta.type) {
    case "replace":
    case "reorder":
      return { forest: buildResourceTreeFromCatalog(nextCatalog), rebuilt: true };
    case "remove": {
      const next = cloneForest(forest);
      for (const resourceId of delta.resourceIds) {
        removeNodeById(next, resourceId);
      }
      return { forest: next, rebuilt: false };
    }
    case "upsert": {
      const next = cloneForest(forest);
      for (const entry of delta.entries) {
        // Drop a prior id that owned this path (synthetic → registry UUID).
        for (const [existingId, existing] of previousCatalog) {
          if (existing.path === entry.path && existingId !== entry.resourceId) {
            removeNodeById(next, existingId);
          }
        }
        removeNodeById(next, entry.resourceId);
        if (!insertCatalogEntry(next, entry, nextCatalog)) {
          return { forest: buildResourceTreeFromCatalog(nextCatalog), rebuilt: true };
        }
      }
      return { forest: next, rebuilt: false };
    }
    default:
      return delta satisfies never;
  }
}

function nodeFromCatalogEntry(entry: CatalogEntry): TreeNode {
  const name = leafName(entry.path);
  if (entry.kind === "folder") {
    return {
      type: "folder",
      name,
      path: entry.path,
      resourceId: entry.resourceId,
      children: [],
    };
  }
  return {
    type: "file",
    name,
    resourceId: entry.resourceId,
    resource: resourceFromCatalogEntry(entry),
  };
}

function sortTree(folder: TreeFolder): void {
  folder.children.sort((a, b) => {
    if (a.type !== b.type) return a.type === "folder" ? -1 : 1;
    return a.name.localeCompare(b.name, undefined, { sensitivity: "base" });
  });
  for (const child of folder.children) {
    if (child.type === "folder") sortTree(child);
  }
}

function cloneForest(forest: readonly TreeNode[]): TreeNode[] {
  return forest.map(cloneNode);
}

function cloneNode(node: TreeNode): TreeNode {
  if (node.type === "file") {
    return {
      type: "file",
      name: node.name,
      resourceId: node.resourceId,
      resource: { ...node.resource },
    };
  }
  return {
    type: "folder",
    name: node.name,
    path: node.path,
    resourceId: node.resourceId,
    children: node.children.map(cloneNode),
  };
}

function removeNodeById(forest: TreeNode[], resourceId: string): boolean {
  for (let index = 0; index < forest.length; index++) {
    const node = forest[index];
    if (node.resourceId === resourceId) {
      forest.splice(index, 1);
      return true;
    }
    if (node.type === "folder" && removeNodeById(node.children, resourceId)) {
      return true;
    }
  }
  return false;
}

function findFolderById(forest: readonly TreeNode[], resourceId: string): TreeFolder | null {
  for (const node of forest) {
    if (node.type === "folder") {
      if (node.resourceId === resourceId) return node;
      const nested = findFolderById(node.children, resourceId);
      if (nested) return nested;
    }
  }
  return null;
}

function findFolderByPath(forest: readonly TreeNode[], path: string): TreeFolder | null {
  for (const node of forest) {
    if (node.type === "folder") {
      if (node.path === path) return node;
      const nested = findFolderByPath(node.children, path);
      if (nested) return nested;
    }
  }
  return null;
}

/**
 * Ensure ancestor folders exist for `path`, creating synthetic folders as needed.
 * Returns the parent folder that should own `path`, or null for workspace root.
 */
function ensureParentFolder(
  forest: TreeNode[],
  path: string,
  catalog: ReadonlyMap<string, CatalogEntry>,
): TreeFolder | null {
  const parentPath = parentPathOf(path);
  if (!parentPath) return null;

  const existing = findFolderByPath(forest, parentPath);
  if (existing) return existing;

  const segments = parentPath.split("/").filter((segment) => segment.length > 0);
  let children = forest;
  let cursor: TreeFolder | null = null;
  for (let depth = 0; depth < segments.length; depth++) {
    const folderPath = segments.slice(0, depth + 1).join("/");
    let folder = children.find(
      (node): node is TreeFolder => node.type === "folder" && node.path === folderPath,
    );
    if (!folder) {
      const catalogId = [...catalog.values()].find(
        (entry) => entry.path === folderPath && entry.kind === "folder",
      )?.resourceId;
      folder = {
        type: "folder",
        name: segments[depth],
        path: folderPath,
        resourceId: catalogId ?? syntheticResourceId(folderPath),
        children: [],
      };
      children.push(folder);
      sortSiblings(children);
    }
    cursor = folder;
    children = folder.children;
  }
  return cursor;
}

function sortSiblings(children: TreeNode[]): void {
  children.sort((a, b) => {
    if (a.type !== b.type) return a.type === "folder" ? -1 : 1;
    return a.name.localeCompare(b.name, undefined, { sensitivity: "base" });
  });
}

function insertCatalogEntry(
  forest: TreeNode[],
  entry: CatalogEntry,
  catalog: ReadonlyMap<string, CatalogEntry>,
): boolean {
  const node = nodeFromCatalogEntry(entry);
  const parentId = parentIdForEntry(entry);

  if (!parentId) {
    forest.push(node);
    sortSiblings(forest);
    return true;
  }

  let parent = findFolderById(forest, parentId);
  if (!parent) {
    parent = ensureParentFolder(forest, entry.path, catalog);
  }
  if (!parent) {
    // Parent should be root but parentId was set — treat as root insert.
    forest.push(node);
    sortSiblings(forest);
    return true;
  }

  parent.children.push(node);
  sortTree(parent);
  return true;
}
