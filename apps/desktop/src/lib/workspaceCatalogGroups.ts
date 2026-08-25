import type { RecentWorkspace } from "./profile";
import {
  workspaceCatalogDisplayName,
  type WorkspaceCatalog,
  type WorkspaceCatalogEntry,
  type WorkspaceSummary,
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

/**
 * Card title for a catalog row: manifest summary when present, else recents, else path leaf.
 * Does not open or scan workspace roots.
 */
export function workspaceCatalogRowTitle(
  entry: WorkspaceCatalogEntry,
  options?: {
    recentTitle?: string | null;
    summary?: WorkspaceSummary | null;
  },
): string {
  const summaryTitle = options?.summary?.manifestPresent ? options.summary.title.trim() : "";
  if (summaryTitle) return summaryTitle;
  const recentTitle = options?.recentTitle?.trim() ?? "";
  if (recentTitle) return recentTitle;
  return workspaceCatalogDisplayName(entry);
}

function rowFromEntry(
  entry: WorkspaceCatalogEntry,
  section: WorkspaceCatalogSection,
  daemonReachable: boolean,
  recentTitle?: string,
  summary?: WorkspaceSummary | null,
): WorkspaceCatalogRow {
  return {
    entry,
    title: workspaceCatalogRowTitle(entry, { recentTitle, summary }),
    location: entry.root,
    status: workspaceCatalogStatus(entry, daemonReachable),
    section,
  };
}

/**
 * Partition registry catalog metadata into pinned / recent / remaining.
 * Never opens or scans workspace roots — titles come from get_workspace_summary
 * (manifest head) when provided, otherwise recents or the path leaf.
 */
export function groupWorkspaceCatalog(args: {
  catalog: WorkspaceCatalog | null | undefined;
  recents: readonly RecentWorkspace[];
  pinnedRoot?: string | null;
  summaries?: ReadonlyMap<string, WorkspaceSummary> | null;
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
      pinned.push(
        rowFromEntry(
          entry,
          "pinned",
          daemonReachable,
          recentTitle,
          args.summaries?.get(entry.workspaceId),
        ),
      );
      pinnedIds.add(entry.workspaceId);
    }
  }

  const recent: WorkspaceCatalogRow[] = [];
  const recentIds = new Set<string>();
  for (const recentEntry of args.recents) {
    const entry = byRoot.get(normalizeWorkspaceRoot(recentEntry.root));
    if (!entry || pinnedIds.has(entry.workspaceId) || recentIds.has(entry.workspaceId)) continue;
    recent.push(
      rowFromEntry(
        entry,
        "recent",
        daemonReachable,
        recentEntry.title,
        args.summaries?.get(entry.workspaceId),
      ),
    );
    recentIds.add(entry.workspaceId);
  }

  const all: WorkspaceCatalogRow[] = workspaces.map((entry) => {
    if (pinnedIds.has(entry.workspaceId)) {
      return pinned.find((row) => row.entry.workspaceId === entry.workspaceId)!;
    }
    if (recentIds.has(entry.workspaceId)) {
      return recent.find((row) => row.entry.workspaceId === entry.workspaceId)!;
    }
    return rowFromEntry(
      entry,
      "all",
      daemonReachable,
      undefined,
      args.summaries?.get(entry.workspaceId),
    );
  });

  return { pinned, recent, all };
}

/** Workspace ids shown on Home (pinned + recent, or the registered list). */
export function visibleWorkspaceCatalogIds(grouped: {
  pinned: readonly WorkspaceCatalogRow[];
  recent: readonly WorkspaceCatalogRow[];
  all: readonly WorkspaceCatalogRow[];
}): string[] {
  const rows =
    grouped.pinned.length === 0 && grouped.recent.length === 0
      ? grouped.all
      : [...grouped.pinned, ...grouped.recent];
  return rows.map((row) => row.entry.workspaceId);
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
