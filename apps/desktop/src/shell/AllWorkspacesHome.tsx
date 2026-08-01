import { Button } from "@lattice/ui";
import { FolderOpen, FolderPlus, DownloadSimple } from "@phosphor-icons/react";
import { useMemo } from "react";

import { useWorkspaceCatalogQuery } from "../query";
import type { RecentWorkspace } from "../lib/profile";
import {
  groupWorkspaceCatalog,
  workspaceCatalogStatusLabel,
  type WorkspaceCatalogRow,
} from "../lib/workspaceCatalogGroups";

function WorkspaceRow({
  row,
  active,
  busy,
  onOpen,
}: {
  row: WorkspaceCatalogRow;
  active: boolean;
  busy: boolean;
  onOpen: (workspaceId: string) => void;
}) {
  return (
    <button
      type="button"
      className={active ? "all-workspaces-row all-workspaces-row-active" : "all-workspaces-row"}
      disabled={busy}
      onClick={() => onOpen(row.entry.workspaceId)}
      title={row.location}
    >
      <span className="all-workspaces-row-main">
        <strong>{row.title}</strong>
        <code>{row.location}</code>
      </span>
      <span className="all-workspaces-row-meta">
        <span className={`all-workspaces-status all-workspaces-status-${row.status}`}>
          {workspaceCatalogStatusLabel(row.status)}
        </span>
        {active ? <span className="all-workspaces-current">Current</span> : null}
      </span>
    </button>
  );
}

function WorkspaceSection({
  label,
  rows,
  activeWorkspaceId,
  busy,
  onOpen,
}: {
  label: string;
  rows: WorkspaceCatalogRow[];
  activeWorkspaceId: string | null;
  busy: boolean;
  onOpen: (workspaceId: string) => void;
}) {
  if (rows.length === 0) return null;
  return (
    <section className="all-workspaces-section">
      <h2>{label}</h2>
      <div className="all-workspaces-list">
        {rows.map((row) => (
          <WorkspaceRow
            key={row.entry.workspaceId}
            row={row}
            active={row.entry.workspaceId === activeWorkspaceId}
            busy={busy}
            onOpen={onOpen}
          />
        ))}
      </div>
    </section>
  );
}

export function AllWorkspacesHome({
  activeWorkspaceId,
  activeWorkspaceTitle,
  pinnedRoot,
  recents,
  busy,
  onOpenById,
  onCreate,
  onOpenFolder,
  onImport,
}: {
  activeWorkspaceId: string | null;
  activeWorkspaceTitle: string;
  pinnedRoot: string | null;
  recents: readonly RecentWorkspace[];
  busy: boolean;
  onOpenById: (workspaceId: string) => void;
  onCreate: () => void;
  onOpenFolder: () => void;
  onImport: () => void;
}) {
  const catalogQuery = useWorkspaceCatalogQuery();
  const grouped = useMemo(
    () =>
      groupWorkspaceCatalog({
        catalog: catalogQuery.data,
        recents,
        pinnedRoot,
      }),
    [catalogQuery.data, pinnedRoot, recents],
  );

  const loading = catalogQuery.isLoading && !catalogQuery.data;
  const empty = !loading && grouped.all.length === 0;

  return (
    <div className="home-dashboard all-workspaces-home">
      <div className="home-welcome">
        <p className="home-eyebrow">All workspaces</p>
        <h1>Workspaces</h1>
        <p>
          Open a registered workspace by id. Lattice lists registry metadata only — it does not
          scan every workspace on Home.
          {activeWorkspaceTitle ? ` Current: ${activeWorkspaceTitle}.` : ""}
        </p>
        <div>
          <Button variant="primary" onClick={onCreate} disabled={busy}>
            <FolderPlus size={14} />
            Create…
          </Button>
          <Button variant="secondary" onClick={onOpenFolder} disabled={busy}>
            <FolderOpen size={14} />
            Open…
          </Button>
          <Button variant="ghost" onClick={onImport} disabled={busy}>
            <DownloadSimple size={14} />
            Import…
          </Button>
        </div>
      </div>

      {loading ? <p className="all-workspaces-empty">Loading workspace catalog…</p> : null}
      {catalogQuery.isError ? (
        <p className="error-text">Could not load workspace catalog.</p>
      ) : null}
      {empty ? (
        <p className="all-workspaces-empty">
          No registered workspaces yet. Create one or open a folder with <code>lattice.yaml</code>.
        </p>
      ) : null}

      <WorkspaceSection
        label="Pinned"
        rows={grouped.pinned}
        activeWorkspaceId={activeWorkspaceId}
        busy={busy}
        onOpen={onOpenById}
      />
      <WorkspaceSection
        label="Recent"
        rows={grouped.recent}
        activeWorkspaceId={activeWorkspaceId}
        busy={busy}
        onOpen={onOpenById}
      />
      {grouped.pinned.length === 0 && grouped.recent.length === 0 && grouped.all.length > 0 ? (
        <WorkspaceSection
          label="Registered"
          rows={grouped.all}
          activeWorkspaceId={activeWorkspaceId}
          busy={busy}
          onOpen={onOpenById}
        />
      ) : null}
    </div>
  );
}
