/**
 * Local-only thread pin preference (no agentd API yet).
 * Shape: workspaceRoot → ordered unique thread ids.
 */

export const PINNED_THREADS_STORAGE_KEY = "lattice.agent.pinnedThreads.v1";

export type PinnedThreadsByWorkspace = Record<string, string[]>;

function normalizeWorkspaceRoot(root: string): string {
  return root.replace(/\\/g, "/").replace(/\/+$/, "");
}

export function parsePinnedThreads(raw: string | null): PinnedThreadsByWorkspace {
  if (!raw) {
    return {};
  }
  try {
    const parsed = JSON.parse(raw) as unknown;
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
      return {};
    }
    const next: PinnedThreadsByWorkspace = {};
    for (const [root, value] of Object.entries(parsed)) {
      if (!Array.isArray(value)) {
        continue;
      }
      const ids = value.filter(
        (entry): entry is string => typeof entry === "string" && entry.trim().length > 0,
      );
      if (ids.length > 0) {
        next[normalizeWorkspaceRoot(root)] = [...new Set(ids.map((id) => id.trim()))];
      }
    }
    return next;
  } catch {
    return {};
  }
}

export function readPinnedThreads(): PinnedThreadsByWorkspace {
  if (typeof localStorage === "undefined") {
    return {};
  }
  try {
    return parsePinnedThreads(localStorage.getItem(PINNED_THREADS_STORAGE_KEY));
  } catch {
    return {};
  }
}

export function persistPinnedThreads(pins: PinnedThreadsByWorkspace): void {
  if (typeof localStorage === "undefined") {
    return;
  }
  try {
    localStorage.setItem(PINNED_THREADS_STORAGE_KEY, JSON.stringify(pins));
  } catch {
    // Ignore quota / private mode failures.
  }
}

export function isThreadPinned(
  pins: PinnedThreadsByWorkspace,
  workspaceRoot: string,
  threadId: string,
): boolean {
  const trimmed = threadId.trim();
  if (!trimmed) {
    return false;
  }
  const list = pins[normalizeWorkspaceRoot(workspaceRoot)] ?? [];
  return list.includes(trimmed);
}

export function togglePinnedThreadId(
  pins: PinnedThreadsByWorkspace,
  workspaceRoot: string,
  threadId: string,
): PinnedThreadsByWorkspace {
  const root = normalizeWorkspaceRoot(workspaceRoot);
  const trimmed = threadId.trim();
  if (!trimmed) {
    return pins;
  }
  const current = pins[root] ?? [];
  const nextList = current.includes(trimmed)
    ? current.filter((id) => id !== trimmed)
    : [...current, trimmed];
  if (nextList.length === 0) {
    const { [root]: _removed, ...rest } = pins;
    return rest;
  }
  return { ...pins, [root]: nextList };
}

export function sortThreadsWithPins<T extends { id: string; updatedAt: number }>(
  threads: readonly T[],
  pinnedIds: readonly string[],
): T[] {
  const pinned = new Set(pinnedIds);
  return threads.slice().sort((a, b) => {
    const aPinned = pinned.has(a.id);
    const bPinned = pinned.has(b.id);
    if (aPinned !== bPinned) {
      return aPinned ? -1 : 1;
    }
    return b.updatedAt - a.updatedAt;
  });
}
