import type { Resource } from "../types";

/** Join a workspace-relative directory and single path segment. */
export function joinWorkspacePath(directory: string, name: string): string {
  const trimmed = name.trim().replace(/^\/+|\/+$/g, "");
  if (!trimmed) return directory;
  const dir = directory.trim().replace(/^\/+|\/+$/g, "");
  return dir ? `${dir}/${trimmed}` : trimmed;
}

export function basename(path: string): string {
  const slash = path.lastIndexOf("/");
  return slash >= 0 ? path.slice(slash + 1) : path;
}

export function parentDirectory(path: string): string {
  const slash = path.lastIndexOf("/");
  return slash >= 0 ? path.slice(0, slash) : "";
}

/** Parent folder for the sidebar "New folder" toolbar control. */
export function newFolderParentPath(
  selected: Resource | null,
  options?: { activeFolderPath?: string | null; fallback?: string },
): string {
  const activeFolderPath = options?.activeFolderPath?.trim();
  if (activeFolderPath) return activeFolderPath;
  if (selected?.kind === "folder") return selected.path;
  if (selected) return parentDirectory(selected.path);
  return options?.fallback ?? "Projects";
}

export function destinationPath(from: string, toDir: string): string {
  const name = basename(from);
  return joinWorkspacePath(toDir, name);
}

/** True when `ancestor` is a strict prefix path segment of `descendant`. */
export function isAncestorPath(ancestor: string, descendant: string): boolean {
  if (!ancestor) return false;
  return descendant === ancestor || descendant.startsWith(`${ancestor}/`);
}

export function resourcePathExists(resources: readonly Resource[], path: string): boolean {
  return resources.some((resource) => resource.path === path);
}

export type MoveResourceValidation =
  | { ok: true; destination: string }
  | { ok: false; reason: string };

/**
 * Client-side guards for sidebar drag-to-move. The command core still
 * validates existence, directory targets, and collisions on invoke.
 */
export function validateMoveResource(
  from: string,
  toDir: string,
  resources: readonly Resource[],
): MoveResourceValidation {
  if (!from.trim()) {
    return { ok: false, reason: "Nothing to move." };
  }
  if (from === toDir) {
    return { ok: false, reason: "Cannot move a resource onto itself." };
  }
  if (isAncestorPath(from, toDir)) {
    return { ok: false, reason: "Cannot move a resource into its own descendant folder." };
  }
  const currentParent = parentDirectory(from);
  if (currentParent === toDir) {
    return { ok: false, reason: "Resource is already in this folder." };
  }
  const destination = destinationPath(from, toDir);
  if (resourcePathExists(resources, destination)) {
    return { ok: false, reason: `${destination} already exists.` };
  }
  return { ok: true, destination };
}

export type MoveResourcesValidation =
  | { ok: true; destinations: string[] }
  | { ok: false; reason: string };

/**
 * Validate a batch move into one folder. Destinations are checked against the
 * workspace plus other batch destinations so two files with the same basename
 * cannot collide mid-batch.
 */
export function validateMoveResources(
  fromPaths: readonly string[],
  toDir: string,
  resources: readonly Resource[],
): MoveResourcesValidation {
  const unique = [...new Set(fromPaths.map((path) => path.trim()).filter(Boolean))];
  if (unique.length === 0) {
    return { ok: false, reason: "Nothing to move." };
  }

  const reserved = new Set(resources.map((resource) => resource.path));
  const destinations: string[] = [];

  for (const from of unique) {
    const validation = validateMoveResource(from, toDir, resources);
    if (!validation.ok) return validation;
    if (reserved.has(validation.destination)) {
      return { ok: false, reason: `${validation.destination} already exists.` };
    }
    reserved.add(validation.destination);
    destinations.push(validation.destination);
  }

  return { ok: true, destinations };
}
