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

/**
 * Build a collapsible folder tree from a flat resource listing (as
 * returned by `list_resources`), grouping by `/`-separated path segments.
 * Within each folder, subfolders sort before files, and both sort
 * alphabetically (case-insensitive) — the sidebar's ordering is otherwise
 * unspecified by the backend scan.
 */
export function buildResourceTree(resources: readonly Resource[]): TreeNode[] {
  const root: TreeFolder = { type: "folder", name: "", path: "", children: [] };

  for (const resource of resources) {
    const segments = resource.path.split("/").filter((segment) => segment.length > 0);
    if (segments.length === 0) continue;

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
