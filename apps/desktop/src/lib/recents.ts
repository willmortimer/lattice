import type { WorkspaceSnapshot } from "../types";

const STORAGE_KEY = "lattice.recentWorkspaces";
const MAX_RECENTS = 8;

export interface RecentWorkspace {
  root: string;
  title: string;
  openedAt: number;
}

function readAll(): RecentWorkspace[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw) as RecentWorkspace[];
    if (!Array.isArray(parsed)) return [];
    return parsed.filter((r) => typeof r?.root === "string" && typeof r?.title === "string");
  } catch {
    return [];
  }
}

function writeAll(entries: RecentWorkspace[]) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(entries.slice(0, MAX_RECENTS)));
}

/** Record a workspace open for the empty-state recents list. */
export function rememberWorkspace(snapshot: WorkspaceSnapshot): void {
  if (typeof localStorage === "undefined") return;
  const next: RecentWorkspace = {
    root: snapshot.root,
    title: snapshot.title,
    openedAt: Date.now(),
  };
  const rest = readAll().filter((r) => r.root !== snapshot.root);
  writeAll([next, ...rest]);
}

export function listRecentWorkspaces(): RecentWorkspace[] {
  if (typeof localStorage === "undefined") return [];
  return readAll();
}

export function clearRecentWorkspaces(): void {
  localStorage.removeItem(STORAGE_KEY);
}
