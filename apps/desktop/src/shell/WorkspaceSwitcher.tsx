import {
  PopoverPopup,
  PopoverPortal,
  PopoverPositioner,
  PopoverRoot,
  PopoverTrigger,
} from "@lattice/ui";
import { CaretDown, FolderOpen, FolderPlus, SquaresFour } from "@phosphor-icons/react";
import { useMemo, useState } from "react";

import { useWorkspaceCatalogQuery } from "../query";
import type { RecentWorkspace } from "../lib/profile";
import {
  filterWorkspaceCatalogRows,
  groupWorkspaceCatalog,
  workspaceCatalogStatusLabel,
  type WorkspaceCatalogRow,
} from "../lib/workspaceCatalogGroups";

function SwitcherSection({
  label,
  rows,
  activeWorkspaceId,
  busy,
  onSelect,
}: {
  label: string;
  rows: WorkspaceCatalogRow[];
  activeWorkspaceId: string | null;
  busy: boolean;
  onSelect: (workspaceId: string) => void;
}) {
  if (rows.length === 0) return null;
  return (
    <div className="workspace-switcher-section">
      <div className="workspace-switcher-section-label">{label}</div>
      <ul className="workspace-switcher-list">
        {rows.map((row) => {
          const active = row.entry.workspaceId === activeWorkspaceId;
          return (
            <li key={row.entry.workspaceId}>
              <button
                type="button"
                className={
                  active
                    ? "workspace-switcher-item workspace-switcher-item-active"
                    : "workspace-switcher-item"
                }
                disabled={busy || active}
                onClick={() => onSelect(row.entry.workspaceId)}
                title={row.location}
              >
                <span className="workspace-switcher-item-title">{row.title}</span>
                <span className="workspace-switcher-item-meta">
                  <code>{row.location}</code>
                  <span>{workspaceCatalogStatusLabel(row.status)}</span>
                </span>
              </button>
            </li>
          );
        })}
      </ul>
    </div>
  );
}

export function WorkspaceSwitcher({
  title,
  activeWorkspaceId,
  pinnedRoot,
  recents,
  busy,
  markGuidanceAnchor = false,
  onOpenById,
  onCreate,
  onOpenFolder,
  onOpenInNewWindow,
  onManage,
}: {
  title: string;
  activeWorkspaceId: string | null;
  pinnedRoot: string | null;
  recents: readonly RecentWorkspace[];
  busy: boolean;
  markGuidanceAnchor?: boolean;
  onOpenById: (workspaceId: string) => void;
  onCreate: () => void;
  onOpenFolder: () => void;
  onOpenInNewWindow: (workspaceId: string) => void;
  onManage: () => void;
}) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
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

  const filteredAll = useMemo(
    () => filterWorkspaceCatalogRows(grouped.all, query),
    [grouped.all, query],
  );
  const searching = query.trim().length > 0;

  return (
    <PopoverRoot
      open={open}
      onOpenChange={(next) => {
        setOpen(next);
        if (!next) setQuery("");
      }}
    >
      <PopoverTrigger
        render={
          <button
            type="button"
            className="workspace-switcher-trigger"
            data-guidance-anchor={markGuidanceAnchor ? "shell.workspace-switcher" : undefined}
            aria-label="Switch workspace"
            title={title}
          >
            <span>{title}</span>
            <CaretDown size={11} />
          </button>
        }
      />
      <PopoverPortal>
        <PopoverPositioner sideOffset={6} align="start">
          <PopoverPopup className="workspace-switcher-popover">
            <input
              className="workspace-switcher-search"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="Search workspaces…"
              aria-label="Search workspaces"
              autoFocus
            />
            <div className="workspace-switcher-body">
              {searching ? (
                <SwitcherSection
                  label="Matches"
                  rows={filteredAll}
                  activeWorkspaceId={activeWorkspaceId}
                  busy={busy}
                  onSelect={(workspaceId) => {
                    setOpen(false);
                    onOpenById(workspaceId);
                  }}
                />
              ) : (
                <>
                  <SwitcherSection
                    label="Pinned"
                    rows={grouped.pinned}
                    activeWorkspaceId={activeWorkspaceId}
                    busy={busy}
                    onSelect={(workspaceId) => {
                      setOpen(false);
                      onOpenById(workspaceId);
                    }}
                  />
                  <SwitcherSection
                    label="Recent"
                    rows={grouped.recent}
                    activeWorkspaceId={activeWorkspaceId}
                    busy={busy}
                    onSelect={(workspaceId) => {
                      setOpen(false);
                      onOpenById(workspaceId);
                    }}
                  />
                  {grouped.pinned.length === 0 && grouped.recent.length === 0 ? (
                    <SwitcherSection
                      label="Registered"
                      rows={grouped.all}
                      activeWorkspaceId={activeWorkspaceId}
                      busy={busy}
                      onSelect={(workspaceId) => {
                        setOpen(false);
                        onOpenById(workspaceId);
                      }}
                    />
                  ) : null}
                </>
              )}
              {!catalogQuery.isLoading && filteredAll.length === 0 ? (
                <p className="workspace-switcher-empty">No matching workspaces.</p>
              ) : null}
            </div>
            <div className="workspace-switcher-actions">
              <button
                type="button"
                className="workspace-switcher-action"
                disabled={busy}
                onClick={() => {
                  setOpen(false);
                  onCreate();
                }}
              >
                <FolderPlus size={13} />
                New workspace…
              </button>
              <button
                type="button"
                className="workspace-switcher-action"
                disabled={busy}
                onClick={() => {
                  setOpen(false);
                  onOpenFolder();
                }}
              >
                <FolderOpen size={13} />
                Open folder…
              </button>
              <button
                type="button"
                className="workspace-switcher-action"
                disabled={!activeWorkspaceId || busy}
                onClick={() => {
                  if (!activeWorkspaceId) return;
                  setOpen(false);
                  onOpenInNewWindow(activeWorkspaceId);
                }}
              >
                <SquaresFour size={13} />
                Open in new window…
              </button>
              <button
                type="button"
                className="workspace-switcher-action"
                onClick={() => {
                  setOpen(false);
                  onManage();
                }}
              >
                Manage workspaces…
              </button>
            </div>
          </PopoverPopup>
        </PopoverPositioner>
      </PopoverPortal>
    </PopoverRoot>
  );
}
