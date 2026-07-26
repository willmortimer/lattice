import { useCallback, useEffect, useMemo, useState } from "react";
import { Button, IconButton } from "@lattice/ui";
import {
  ArrowClockwise,
  GithubLogo,
  GitlabLogo,
  LinkBreak,
  Plus,
  WarningCircle,
} from "@phosphor-icons/react";

import { presentAuthorizeUrl } from "./lib/authPresenter";
import {
  githubConnectRepo,
  githubDisconnectRepo,
  githubListBindings,
  githubListCheckoutTree,
  githubListRepos,
  githubOauthBegin,
  githubOauthFinish,
  githubReadCheckoutFile,
  githubRefreshRepo,
  type CheckoutEntry,
  type GithubRepoSummary,
} from "./lib/github";
import {
  gitlabConnectRepo,
  gitlabDisconnectRepo,
  gitlabListBindings,
  gitlabListCheckoutTree,
  gitlabListProjects,
  gitlabOauthBegin,
  gitlabOauthFinish,
  gitlabReadCheckoutFile,
  gitlabRefreshRepo,
  type GitlabProjectSummary,
} from "./lib/gitlab";
import { hasTauri } from "./lib/ipc";

export type ConnectedProvider = "github" | "gitlab";

export interface ConnectedRootsProps {
  workspaceRoot: string;
  onOpenFile: (detail: {
    provider: ConnectedProvider;
    bindingId: string;
    owner: string;
    repo: string;
    path: string;
    stale: boolean;
  }) => void;
  onError: (message: string) => void;
}

type ListedRepo =
  | { provider: "github"; repo: GithubRepoSummary }
  | { provider: "gitlab"; repo: GitlabProjectSummary };

type ConnectPhase =
  | { step: "idle" }
  | { step: "pick-provider" }
  | { step: "waiting-browser"; provider: ConnectedProvider }
  | { step: "repos"; provider: ConnectedProvider; accessToken: string; repos: ListedRepo[] }
  | { step: "cloning"; fullName: string };

type UnifiedBinding = {
  provider: ConnectedProvider;
  id: string;
  owner: string;
  repo: string;
  label: string;
  stale: boolean;
  lastError?: string | null;
};

function basename(path: string): string {
  const parts = path.split("/");
  return parts[parts.length - 1] || path;
}

function parentPath(path: string): string {
  const idx = path.lastIndexOf("/");
  return idx === -1 ? "" : path.slice(0, idx);
}

function treeKey(provider: ConnectedProvider, bindingId: string): string {
  return `${provider}:${bindingId}`;
}

/** Connected GitHub/GitLab extracts: browse in-app; connect via system-browser OAuth. */
export function ConnectedRoots({ workspaceRoot, onOpenFile, onError }: ConnectedRootsProps) {
  const [bindings, setBindings] = useState<UnifiedBinding[]>([]);
  const [expanded, setExpanded] = useState<ReadonlySet<string>>(new Set());
  const [trees, setTrees] = useState<Record<string, CheckoutEntry[]>>({});
  const [collapsedFolders, setCollapsedFolders] = useState<ReadonlySet<string>>(new Set());
  const [phase, setPhase] = useState<ConnectPhase>({ step: "idle" });
  const [busy, setBusy] = useState(false);

  const reloadBindings = useCallback(async () => {
    if (!hasTauri) return;
    try {
      const [github, gitlab] = await Promise.all([
        githubListBindings(workspaceRoot),
        gitlabListBindings(workspaceRoot),
      ]);
      const next: UnifiedBinding[] = [
        ...github.map((summary) => ({
          provider: "github" as const,
          id: summary.binding.id,
          owner: summary.binding.owner,
          repo: summary.binding.repo,
          label: `${summary.binding.owner}/${summary.binding.repo}`,
          stale: summary.stale,
          lastError: summary.binding.last_error,
        })),
        ...gitlab.map((summary) => ({
          provider: "gitlab" as const,
          id: summary.binding.id,
          owner: summary.binding.owner,
          repo: summary.binding.repo,
          label: summary.binding.path_with_namespace,
          stale: summary.stale,
          lastError: summary.binding.last_error,
        })),
      ];
      next.sort((a, b) => a.label.localeCompare(b.label));
      setBindings(next);
    } catch (error) {
      onError(error instanceof Error ? error.message : String(error));
    }
  }, [onError, workspaceRoot]);

  useEffect(() => {
    void reloadBindings();
  }, [reloadBindings]);

  const startConnect = async (provider: ConnectedProvider) => {
    setBusy(true);
    setPhase({ step: "waiting-browser", provider });
    try {
      if (provider === "github") {
        const start = await githubOauthBegin();
        await presentAuthorizeUrl(start.authorizeUrl);
        const accessToken = await githubOauthFinish(start.sessionId);
        const repos = await githubListRepos(accessToken);
        setPhase({
          step: "repos",
          provider,
          accessToken,
          repos: repos.map((repo) => ({ provider, repo })),
        });
      } else {
        const start = await gitlabOauthBegin();
        await presentAuthorizeUrl(start.authorizeUrl);
        const accessToken = await gitlabOauthFinish(start.sessionId);
        const projects = await gitlabListProjects(accessToken);
        setPhase({
          step: "repos",
          provider,
          accessToken,
          repos: projects.map((repo) => ({ provider, repo })),
        });
      }
    } catch (error) {
      setPhase({ step: "idle" });
      onError(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  };

  const connectListed = async (listed: ListedRepo, accessToken: string) => {
    const fullName =
      listed.provider === "github" ? listed.repo.full_name : listed.repo.path_with_namespace;
    setPhase({ step: "cloning", fullName });
    setBusy(true);
    try {
      if (listed.provider === "github") {
        await githubConnectRepo({
          root: workspaceRoot,
          accessToken,
          owner: listed.repo.owner,
          repo: listed.repo.name,
          repoId: listed.repo.id,
          defaultBranch: listed.repo.default_branch,
          installationId: listed.repo.installation_id,
        });
      } else {
        await gitlabConnectRepo({
          root: workspaceRoot,
          accessToken,
          pathWithNamespace: listed.repo.path_with_namespace,
          projectId: listed.repo.id,
          defaultBranch: listed.repo.default_branch,
        });
      }
      setPhase({ step: "idle" });
      await reloadBindings();
    } catch (error) {
      setPhase({ step: "idle" });
      onError(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  };

  const loadTree = async (provider: ConnectedProvider, bindingId: string) => {
    const key = treeKey(provider, bindingId);
    if (provider === "github") {
      return githubListCheckoutTree(workspaceRoot, bindingId).then((entries) => {
        setTrees((current) => ({ ...current, [key]: entries }));
      });
    }
    return gitlabListCheckoutTree(workspaceRoot, bindingId).then((entries) => {
      setTrees((current) => ({ ...current, [key]: entries }));
    });
  };

  const toggleBinding = async (binding: UnifiedBinding) => {
    const key = treeKey(binding.provider, binding.id);
    const next = new Set(expanded);
    if (next.has(key)) {
      next.delete(key);
      setExpanded(next);
      return;
    }
    next.add(key);
    setExpanded(next);
    if (!trees[key]) {
      try {
        await loadTree(binding.provider, binding.id);
      } catch (error) {
        onError(error instanceof Error ? error.message : String(error));
      }
    }
  };

  const folderChildren = useMemo(() => {
    const map: Record<string, Record<string, CheckoutEntry[]>> = {};
    for (const [bindingKey, entries] of Object.entries(trees)) {
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
      map[bindingKey] = byParent;
    }
    return map;
  }, [trees]);

  const renderTree = (binding: UnifiedBinding, parent: string, depth: number) => {
    const key = treeKey(binding.provider, binding.id);
    const children = folderChildren[key]?.[parent] ?? [];
    return children.map((entry) => {
      const rowKey = `${key}:${entry.path}`;
      if (entry.is_dir) {
        const collapsed = collapsedFolders.has(rowKey);
        return (
          <div key={rowKey}>
            <button
              type="button"
              className="connected-tree-row"
              style={{ paddingLeft: 8 + depth * 12 }}
              onClick={() => {
                const next = new Set(collapsedFolders);
                if (next.has(rowKey)) next.delete(rowKey);
                else next.add(rowKey);
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
          key={rowKey}
          type="button"
          className="connected-tree-row connected-tree-file"
          style={{ paddingLeft: 8 + depth * 12 }}
          onClick={() =>
            onOpenFile({
              provider: binding.provider,
              bindingId: binding.id,
              owner: binding.owner,
              repo: binding.repo,
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
        <IconButton
          label="Connect repository"
          disabled={busy}
          onClick={() => setPhase({ step: "pick-provider" })}
        >
          <Plus size={14} />
        </IconButton>
      </header>

      {phase.step === "pick-provider" && (
        <div className="connected-auth-panel">
          <p className="connected-auth-label">Connect with</p>
          <div className="connected-provider-actions">
            <Button
              size="sm"
              disabled={busy}
              onClick={() => void startConnect("github")}
            >
              <GithubLogo size={14} /> GitHub
            </Button>
            <Button
              size="sm"
              disabled={busy}
              onClick={() => void startConnect("gitlab")}
            >
              <GitlabLogo size={14} /> GitLab
            </Button>
          </div>
          <Button variant="ghost" size="sm" onClick={() => setPhase({ step: "idle" })}>
            Cancel
          </Button>
        </div>
      )}

      {phase.step === "waiting-browser" && (
        <div className="connected-auth-panel">
          <p>
            Complete {phase.provider === "github" ? "GitHub" : "GitLab"} authorization in your
            browser, then return here.
          </p>
          <Button variant="ghost" size="sm" onClick={() => setPhase({ step: "idle" })}>
            Cancel
          </Button>
        </div>
      )}

      {phase.step === "repos" && (
        <div className="connected-auth-panel">
          <p className="connected-auth-label">Choose a repository</p>
          <ul className="connected-repo-list">
            {phase.repos.map((listed) => {
              const id =
                listed.provider === "github"
                  ? `gh-${listed.repo.id}`
                  : `gl-${listed.repo.id}`;
              const label =
                listed.provider === "github"
                  ? listed.repo.full_name
                  : listed.repo.path_with_namespace;
              const isPrivate = listed.repo.private;
              return (
                <li key={id}>
                  <button
                    type="button"
                    className="connected-repo-item"
                    disabled={busy}
                    onClick={() => void connectListed(listed, phase.accessToken)}
                  >
                    {label}
                    {isPrivate ? " (private)" : ""}
                  </button>
                </li>
              );
            })}
          </ul>
          <Button variant="ghost" size="sm" onClick={() => setPhase({ step: "idle" })}>
            Cancel
          </Button>
        </div>
      )}

      {phase.step === "cloning" && (
        <div className="connected-auth-panel">
          <p>Cloning {phase.fullName} (shallow, read-only)…</p>
        </div>
      )}

      {bindings.length === 0 && phase.step === "idle" && (
        <p className="connected-empty">
          Connect GitHub or GitLab (browser OAuth), or use{" "}
          <code>lattice github</code> / <code>lattice gitlab</code> from the CLI.
        </p>
      )}

      <ul className="connected-binding-list">
        {bindings.map((binding) => {
          const key = treeKey(binding.provider, binding.id);
          const open = expanded.has(key);
          return (
            <li key={key} className="connected-binding">
              <div className="connected-binding-row">
                <button
                  type="button"
                  className="connected-binding-toggle"
                  onClick={() => void toggleBinding(binding)}
                >
                  {open ? "▾" : "▸"}{" "}
                  {binding.provider === "github" ? (
                    <GithubLogo size={12} />
                  ) : (
                    <GitlabLogo size={12} />
                  )}{" "}
                  {binding.label}
                </button>
                {binding.stale && (
                  <span
                    className="connected-stale"
                    title={binding.lastError ?? "Offline or stale"}
                  >
                    <WarningCircle size={12} /> Stale
                  </span>
                )}
                <IconButton
                  label="Refresh"
                  onClick={() => {
                    const refresh =
                      binding.provider === "github"
                        ? githubRefreshRepo(workspaceRoot, binding.id)
                        : gitlabRefreshRepo(workspaceRoot, binding.id);
                    void refresh
                      .then(async () => {
                        await reloadBindings();
                        await loadTree(binding.provider, binding.id);
                      })
                      .catch((error) => {
                        onError(error instanceof Error ? error.message : String(error));
                        void reloadBindings();
                      });
                  }}
                >
                  <ArrowClockwise size={12} />
                </IconButton>
                <IconButton
                  label="Disconnect"
                  onClick={() => {
                    const disconnect =
                      binding.provider === "github"
                        ? githubDisconnectRepo(workspaceRoot, binding.id)
                        : gitlabDisconnectRepo(workspaceRoot, binding.id);
                    void disconnect
                      .then(() => reloadBindings())
                      .catch((error) =>
                        onError(error instanceof Error ? error.message : String(error)),
                      );
                  }}
                >
                  <LinkBreak size={12} />
                </IconButton>
              </div>
              {open && <div className="connected-tree">{renderTree(binding, "", 1)}</div>}
            </li>
          );
        })}
      </ul>
    </section>
  );
}

export async function readConnectedCheckoutFile(
  provider: ConnectedProvider,
  root: string,
  bindingId: string,
  relPath: string,
) {
  if (provider === "github") {
    return githubReadCheckoutFile(root, bindingId, relPath);
  }
  return gitlabReadCheckoutFile(root, bindingId, relPath);
}
