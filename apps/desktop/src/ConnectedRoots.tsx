import { useCallback, useEffect, useMemo, useState } from "react";
import { IconButton } from "@lattice/ui";
import { ArrowClockwise, GithubLogo, LinkBreak, WarningCircle } from "@phosphor-icons/react";

import {
  githubDisconnectRepo,
  githubListBindings,
  githubListCheckoutTree,
  githubRefreshRepo,
  type CheckoutEntry,
  type ConnectedRepoSummary,
} from "./lib/github";
import { hasTauri } from "./lib/ipc";

export interface ConnectedRootsProps {
  workspaceRoot: string;
  onOpenFile: (detail: {
    bindingId: string;
    owner: string;
    repo: string;
    path: string;
    stale: boolean;
  }) => void;
  onError: (message: string) => void;
}

function basename(path: string): string {
  const parts = path.split("/");
  return parts[parts.length - 1] || path;
}

function parentPath(path: string): string {
  const idx = path.lastIndexOf("/");
  return idx === -1 ? "" : path.slice(0, idx);
}

/** Browse Connected GitHub extracts. Connect/auth is CLI-only (`lattice github`). */
export function ConnectedRoots({ workspaceRoot, onOpenFile, onError }: ConnectedRootsProps) {
  const [bindings, setBindings] = useState<ConnectedRepoSummary[]>([]);
  const [expanded, setExpanded] = useState<ReadonlySet<string>>(new Set());
  const [trees, setTrees] = useState<Record<string, CheckoutEntry[]>>({});
  const [collapsedFolders, setCollapsedFolders] = useState<ReadonlySet<string>>(new Set());

  const reloadBindings = useCallback(async () => {
    if (!hasTauri) return;
    try {
      const next = await githubListBindings(workspaceRoot);
      setBindings(next);
    } catch (error) {
      onError(error instanceof Error ? error.message : String(error));
    }
  }, [onError, workspaceRoot]);

  useEffect(() => {
    void reloadBindings();
  }, [reloadBindings]);

  const toggleBinding = async (bindingId: string) => {
    const next = new Set(expanded);
    if (next.has(bindingId)) {
      next.delete(bindingId);
      setExpanded(next);
      return;
    }
    next.add(bindingId);
    setExpanded(next);
    if (!trees[bindingId]) {
      try {
        const entries = await githubListCheckoutTree(workspaceRoot, bindingId);
        setTrees((current) => ({ ...current, [bindingId]: entries }));
      } catch (error) {
        onError(error instanceof Error ? error.message : String(error));
      }
    }
  };

  const folderChildren = useMemo(() => {
    const map: Record<string, Record<string, CheckoutEntry[]>> = {};
    for (const [bindingId, entries] of Object.entries(trees)) {
      const byParent: Record<string, CheckoutEntry[]> = { "": [] };
      for (const entry of entries) {
        const parent = parentPath(entry.path);
        if (!byParent[parent]) byParent[parent] = [];
        const depth = entry.path.split("/").length;
        const parentDepth = parent ? parent.split("/").length : 0;
        if (depth === parentDepth + 1) {
          byParent[parent].push(entry);
        }
      }
      for (const list of Object.values(byParent)) {
        list.sort((a, b) => {
          if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1;
          return a.path.localeCompare(b.path);
        });
      }
      map[bindingId] = byParent;
    }
    return map;
  }, [trees]);

  const renderTree = (binding: ConnectedRepoSummary, parent: string, depth: number) => {
    const children = folderChildren[binding.binding.id]?.[parent] ?? [];
    return children.map((entry) => {
      const key = `${binding.binding.id}:${entry.path}`;
      if (entry.is_dir) {
        const collapsed = collapsedFolders.has(key);
        return (
          <div key={key}>
            <button
              type="button"
              className="connected-tree-row"
              style={{ paddingLeft: 8 + depth * 12 }}
              onClick={() => {
                const next = new Set(collapsedFolders);
                if (next.has(key)) next.delete(key);
                else next.add(key);
                setCollapsedFolders(next);
              }}
            >
              {collapsed ? "▸" : "▾"} {basename(entry.path)}
            </button>
            {!collapsed && renderTree(binding, entry.path, depth + 1)}
          </div>
        );
      }
      return (
        <button
          key={key}
          type="button"
          className="connected-tree-row connected-tree-file"
          style={{ paddingLeft: 8 + depth * 12 }}
          onClick={() =>
            onOpenFile({
              bindingId: binding.binding.id,
              owner: binding.binding.owner,
              repo: binding.binding.repo,
              path: entry.path,
              stale: binding.stale,
            })
          }
        >
          {basename(entry.path)}
        </button>
      );
    });
  };

  if (!hasTauri) {
    return null;
  }

  return (
    <section className="connected-roots" aria-label="Connected repositories">
      <header className="connected-roots-head">
        <span className="connected-roots-title">
          <GithubLogo size={14} /> Connected
        </span>
      </header>

      {bindings.length === 0 && (
        <p className="connected-empty">
          No GitHub extracts yet. Connect from the CLI:{" "}
          <code>lattice github login</code> then <code>lattice github connect owner/repo</code>
        </p>
      )}

      <ul className="connected-binding-list">
        {bindings.map((summary) => {
          const id = summary.binding.id;
          const open = expanded.has(id);
          return (
            <li key={id} className="connected-binding">
              <div className="connected-binding-row">
                <button type="button" className="connected-binding-toggle" onClick={() => void toggleBinding(id)}>
                  {open ? "▾" : "▸"} {summary.binding.owner}/{summary.binding.repo}
                </button>
                {summary.stale && (
                  <span className="connected-stale" title={summary.binding.last_error ?? "Offline or stale"}>
                    <WarningCircle size={12} /> Stale
                  </span>
                )}
                <IconButton
                  label="Refresh"
                  onClick={() =>
                    void githubRefreshRepo(workspaceRoot, id)
                      .then(async () => {
                        await reloadBindings();
                        const entries = await githubListCheckoutTree(workspaceRoot, id);
                        setTrees((current) => ({ ...current, [id]: entries }));
                      })
                      .catch((error) => {
                        onError(error instanceof Error ? error.message : String(error));
                        void reloadBindings();
                      })
                  }
                >
                  <ArrowClockwise size={12} />
                </IconButton>
                <IconButton
                  label="Disconnect"
                  onClick={() =>
                    void githubDisconnectRepo(workspaceRoot, id)
                      .then(() => reloadBindings())
                      .catch((error) => onError(error instanceof Error ? error.message : String(error)))
                  }
                >
                  <LinkBreak size={12} />
                </IconButton>
              </div>
              {open && <div className="connected-tree">{renderTree(summary, "", 1)}</div>}
            </li>
          );
        })}
      </ul>
    </section>
  );
}
