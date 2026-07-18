/**
 * Remap a workspace-relative path after a rename/move (or its undo).
 * Exact matches and descendants under `from/` are rewritten with the `to` prefix.
 */
export function remapWorkspacePath(path: string, from: string, to: string): string {
  if (path === from) return to;
  if (from && path.startsWith(`${from}/`)) {
    return `${to}${path.slice(from.length)}`;
  }
  return path;
}

export interface PathRemap {
  from: string;
  to: string;
}

/** Apply remaps in order. Later remaps see earlier results. */
export function applyPathRemaps(path: string, remaps: readonly PathRemap[]): string {
  return remaps.reduce(
    (current, remap) => remapWorkspacePath(current, remap.from, remap.to),
    path,
  );
}
