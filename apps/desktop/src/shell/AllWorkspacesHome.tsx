import { Button } from "@lattice/ui";
import { FolderOpen, FolderPlus, DownloadSimple } from "@phosphor-icons/react";
import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";

import { inBrowser } from "../demo";
import { listAccountCloudWorkspaces, type AccountCloudWorkspace } from "../lib/encryptedBackup";
import type { RecentWorkspace } from "../lib/profile";
import { queryKeys, useCloudSessionQuery, useWorkspaceCatalogQuery, useWorkspaceSummaryQueries } from "../query";
import {
  groupWorkspaceCatalog,
  visibleWorkspaceCatalogIds,
  workspaceCatalogStatusLabel,
  type WorkspaceCatalogRow,
} from "../lib/workspaceCatalogGroups";
import { cloudWorkspacesNotOnThisDevice } from "./emptyCloudRestore";

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

function CloudWorkspaceRow({
  workspace,
  busy,
  onDownload,
}: {
  workspace: AccountCloudWorkspace;
  busy: boolean;
  onDownload: (cloudWorkspaceId: string) => void;
}) {
  return (
    <div className="all-workspaces-row all-workspaces-row-static">
      <span className="all-workspaces-row-main">
        <strong>{workspace.name.trim() || workspace.id}</strong>
        <code>Not on this device</code>
      </span>
      <span className="all-workspaces-row-meta">
        <Button
          variant="secondary"
          onClick={() => onDownload(workspace.id)}
          disabled={busy}
        >
          Download…
        </Button>
      </span>
    </div>
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
  onDownloadCloudWorkspace,
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
  onDownloadCloudWorkspace: (cloudWorkspaceId: string) => void;
}) {
  const catalogQuery = useWorkspaceCatalogQuery();
  const { data: cloudSession } = useCloudSessionQuery();
  const signedIn = Boolean(cloudSession?.signedIn);
  const cloudQuery = useQuery({
    queryKey: queryKeys.accountCloudWorkspaces(),
    queryFn: listAccountCloudWorkspaces,
    enabled: signedIn && !inBrowser,
  });
  const groupedMeta = useMemo(
    () =>
      groupWorkspaceCatalog({
        catalog: catalogQuery.data,
        recents,
        pinnedRoot,
      }),
    [catalogQuery.data, pinnedRoot, recents],
  );
  const summaryIds = useMemo(() => visibleWorkspaceCatalogIds(groupedMeta), [groupedMeta]);
  const summaries = useWorkspaceSummaryQueries(summaryIds);
  const grouped = useMemo(
    () =>
      groupWorkspaceCatalog({
        catalog: catalogQuery.data,
        recents,
        pinnedRoot,
        summaries,
      }),
    [catalogQuery.data, pinnedRoot, recents, summaries],
  );
  const localWorkspaceIds = useMemo(
    () => new Set((catalogQuery.data?.workspaces ?? []).map((entry) => entry.workspaceId)),
    [catalogQuery.data],
  );
  const missingCloudWorkspaces = useMemo(
    () => cloudWorkspacesNotOnThisDevice(cloudQuery.data ?? [], localWorkspaceIds),
    [cloudQuery.data, localWorkspaceIds],
  );

  const loading = catalogQuery.isLoading && !catalogQuery.data;
  const empty = !loading && grouped.all.length === 0;
  const activeSummary = activeWorkspaceId ? summaries.get(activeWorkspaceId) : undefined;
  const currentTitle =
    (activeSummary?.manifestPresent ? activeSummary.title.trim() : "") || activeWorkspaceTitle;

  return (
    <div className="home-dashboard all-workspaces-home">
      <div className="home-welcome">
        <p className="home-eyebrow">All workspaces</p>
        <h1>Workspaces</h1>
        <p>
          Open a registered workspace by id. Lattice lists registry metadata only — it does not
          scan every workspace on Home.
          {currentTitle ? ` Current: ${currentTitle}.` : ""}
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
            Download…
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

      {signedIn && missingCloudWorkspaces.length > 0 ? (
        <section className="all-workspaces-section">
          <h2>Cloud (not on this device)</h2>
          <p className="all-workspaces-empty">
            Download restores an encrypted backup into a folder on this computer, then opens it.
          </p>
          <div className="all-workspaces-list">
            {missingCloudWorkspaces.map((workspace) => (
              <CloudWorkspaceRow
                key={workspace.id}
                workspace={workspace}
                busy={busy}
                onDownload={onDownloadCloudWorkspace}
              />
            ))}
          </div>
        </section>
      ) : null}
      {signedIn && cloudQuery.isError ? (
        <p className="error-text">Could not list cloud workspaces.</p>
      ) : null}
    </div>
  );
}
