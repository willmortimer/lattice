import type { Resource } from "../types";

/** A folder grouping other nodes, keyed by its full path from the workspace root. */
export interface TreeFolder {
  type: "folder";
  name: string;
  path: string;
  children: TreeNode[];
}

/** A leaf node wrapping one resource. */
export interface TreeFile {
  type: "file";
  name: string;
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
      name: string;
      folder: TreeFolder;
    }
  | {
      type: "file";
      depth: number;
      path: string;
      name: string;
      resource: Resource;
    }
  | {
      type: "empty-folder";
      depth: number;
      path: string;
      name: string;
      folder: TreeFolder;
    };

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
          name: node.name,
          resource: node.resource,
        });
        continue;
      }

      rows.push({
        type: "folder",
        depth,
        path: node.path,
        name: node.name,
        folder: node,
      });

      if (collapsed.has(node.path)) continue;

      if (node.children.length === 0) {
        rows.push({
          type: "empty-folder",
          depth: depth + 1,
          path: node.path,
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
 */
export function buildResourceTree(resources: readonly Resource[]): TreeNode[] {
  const root: TreeFolder = { type: "folder", name: "", path: "", children: [] };

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
          folder = { type: "folder", name, path, children: [] };
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
        folder = { type: "folder", name, path, children: [] };
        cursor.children.push(folder);
      }
      cursor = folder;
    }

    const name = segments[segments.length - 1];
    cursor.children.push({ type: "file", name, resource });
  }

  sortTree(root);
  return root.children;
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
