import type { RecentWorkspace } from "./profile";
import {
  workspaceCatalogDisplayName,
  type WorkspaceCatalog,
  type WorkspaceCatalogEntry,
} from "./workspaceCatalog";

export type WorkspaceCatalogStatus = "available" | "remote" | "offline";

export type WorkspaceCatalogSection = "pinned" | "recent" | "all";

export interface WorkspaceCatalogRow {
  entry: WorkspaceCatalogEntry;
  title: string;
  location: string;
  status: WorkspaceCatalogStatus;
  section: WorkspaceCatalogSection;
}

export function normalizeWorkspaceRoot(root: string): string {
  return root.replace(/\\/g, "/").replace(/\/+$/, "");
}

export function workspaceCatalogStatus(
  entry: WorkspaceCatalogEntry,
  daemonReachable: boolean,
): WorkspaceCatalogStatus {
  if (entry.remoteAccessEnabled) {
    return daemonReachable ? "remote" : "offline";
  }
  return "available";
}

export function workspaceCatalogStatusLabel(status: WorkspaceCatalogStatus): string {
  switch (status) {
    case "remote":
      return "Remote";
    case "offline":
      return "Offline";
    case "available":
      return "Available";
    default: {
      const _exhaustive: never = status;
      return _exhaustive;
    }
  }
}

function rowFromEntry(
  entry: WorkspaceCatalogEntry,
  section: WorkspaceCatalogSection,
  daemonReachable: boolean,
  titleOverride?: string,
): WorkspaceCatalogRow {
  return {
    entry,
    title: titleOverride?.trim() || workspaceCatalogDisplayName(entry),
    location: entry.root,
    status: workspaceCatalogStatus(entry, daemonReachable),
    section,
  };
}

/**
 * Partition registry catalog metadata into pinned / recent / remaining.
 * Never opens or scans workspace roots — titles come from path leaf or recent titles.
 */
export function groupWorkspaceCatalog(args: {
  catalog: WorkspaceCatalog | null | undefined;
  recents: readonly RecentWorkspace[];
  pinnedRoot?: string | null;
}): {
  pinned: WorkspaceCatalogRow[];
  recent: WorkspaceCatalogRow[];
  all: WorkspaceCatalogRow[];
} {
  const workspaces = args.catalog?.workspaces ?? [];
  const daemonReachable = args.catalog?.daemonReachable ?? false;
  const byRoot = new Map(
    workspaces.map((entry) => [normalizeWorkspaceRoot(entry.root), entry] as const),
  );

  const pinnedRoot = args.pinnedRoot ? normalizeWorkspaceRoot(args.pinnedRoot) : null;
  const pinned: WorkspaceCatalogRow[] = [];
  const pinnedIds = new Set<string>();

  if (pinnedRoot) {
    const entry = byRoot.get(pinnedRoot);
    if (entry) {
      const recentTitle = args.recents.find(
        (recent) => normalizeWorkspaceRoot(recent.root) === pinnedRoot,
      )?.title;
      pinned.push(rowFromEntry(entry, "pinned", daemonReachable, recentTitle));
      pinnedIds.add(entry.workspaceId);
    }
  }

  const recent: WorkspaceCatalogRow[] = [];
  const recentIds = new Set<string>();
  for (const recentEntry of args.recents) {
    const entry = byRoot.get(normalizeWorkspaceRoot(recentEntry.root));
    if (!entry || pinnedIds.has(entry.workspaceId) || recentIds.has(entry.workspaceId)) continue;
    recent.push(rowFromEntry(entry, "recent", daemonReachable, recentEntry.title));
    recentIds.add(entry.workspaceId);
  }

  const all: WorkspaceCatalogRow[] = workspaces.map((entry) => {
    if (pinnedIds.has(entry.workspaceId)) {
      return pinned.find((row) => row.entry.workspaceId === entry.workspaceId)!;
    }
    if (recentIds.has(entry.workspaceId)) {
      return recent.find((row) => row.entry.workspaceId === entry.workspaceId)!;
    }
    return rowFromEntry(entry, "all", daemonReachable);
  });

  return { pinned, recent, all };
}

export function filterWorkspaceCatalogRows(
  rows: readonly WorkspaceCatalogRow[],
  query: string,
): WorkspaceCatalogRow[] {
  const needle = query.trim().toLowerCase();
  if (!needle) return [...rows];
  return rows.filter((row) => {
    return (
      row.title.toLowerCase().includes(needle) ||
      row.location.toLowerCase().includes(needle) ||
      row.entry.workspaceId.toLowerCase().includes(needle)
    );
  });
}
