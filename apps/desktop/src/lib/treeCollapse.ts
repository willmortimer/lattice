/** Per-workspace collapsed folder paths persisted in the Lattice profile UI store. */
export type ResourceTreeCollapseState = Record<string, readonly string[]>;

const STORAGE_KEY = "resource-tree-collapsed";

export function resourceTreeCollapseStorageKey(): string {
  return STORAGE_KEY;
}

export function parseResourceTreeCollapseState(
  raw: string | null | undefined,
): ResourceTreeCollapseState {
  if (!raw) return {};
  try {
    const parsed: unknown = JSON.parse(raw);
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) return {};
    const next: Record<string, string[]> = {};
    for (const [workspaceKey, value] of Object.entries(parsed)) {
      if (!Array.isArray(value)) continue;
      const paths = value.filter((entry): entry is string => typeof entry === "string");
      if (paths.length > 0) next[workspaceKey] = paths;
    }
    return next;
  } catch {
    return {};
  }
}

export function serializeResourceTreeCollapseState(state: ResourceTreeCollapseState): string {
  const payload: Record<string, string[]> = {};
  for (const [workspaceKey, paths] of Object.entries(state)) {
    if (paths.length === 0) continue;
    payload[workspaceKey] = [...paths];
  }
  return JSON.stringify(payload);
}

export function collapsedPathsForWorkspace(
  state: ResourceTreeCollapseState,
  workspaceKey: string | null | undefined,
): ReadonlySet<string> {
  if (!workspaceKey) return new Set();
  return new Set(state[workspaceKey] ?? []);
}

export function updateCollapsedPathsForWorkspace(
  state: ResourceTreeCollapseState,
  workspaceKey: string,
  paths: ReadonlySet<string> | readonly string[],
): ResourceTreeCollapseState {
  const nextPaths = [...paths];
  if (nextPaths.length === 0) {
    const { [workspaceKey]: _removed, ...rest } = state;
    return rest;
  }
  return { ...state, [workspaceKey]: nextPaths };
}
