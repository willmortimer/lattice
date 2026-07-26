import { invoke } from "./ipc";
import type { CheckoutEntry, CheckoutFile } from "./github";

export interface GitlabOAuthStartResult {
  sessionId: string;
  authorizeUrl: string;
  redirectUri: string;
  redirectMode: string;
}

export interface GitlabProjectSummary {
  id: number;
  path_with_namespace: string;
  owner: string;
  name: string;
  default_branch: string;
  private: boolean;
  clone_url: string;
}

export interface GitlabRepoBinding {
  kind: string;
  id: string;
  path_with_namespace: string;
  owner: string;
  repo: string;
  project_id: number;
  default_branch: string;
  head_sha?: string | null;
  mode: "read";
  credentials: { provider: string; key: string };
  extract: { strategy: string; depth: number; path: string };
  capabilities: string[];
  last_refreshed_at?: string | null;
  stale?: boolean | null;
  last_error?: string | null;
}

export interface ConnectedGitlabRepoSummary {
  binding: GitlabRepoBinding;
  checkout_exists: boolean;
  stale: boolean;
}

export async function gitlabOauthBegin(): Promise<GitlabOAuthStartResult> {
  return invoke<GitlabOAuthStartResult>("gitlab_oauth_begin");
}

export async function gitlabOauthFinish(sessionId: string): Promise<string> {
  return invoke<string>("gitlab_oauth_finish", { sessionId });
}

export async function gitlabListProjects(
  accessToken: string,
): Promise<GitlabProjectSummary[]> {
  return invoke<GitlabProjectSummary[]>("gitlab_list_projects", { accessToken });
}

export async function gitlabConnectRepo(input: {
  root: string;
  accessToken: string;
  pathWithNamespace: string;
  projectId: number;
  defaultBranch: string;
}): Promise<ConnectedGitlabRepoSummary> {
  return invoke<ConnectedGitlabRepoSummary>("gitlab_connect_repo", {
    root: input.root,
    accessToken: input.accessToken,
    pathWithNamespace: input.pathWithNamespace,
    projectId: input.projectId,
    defaultBranch: input.defaultBranch,
  });
}

export async function gitlabListBindings(root: string): Promise<ConnectedGitlabRepoSummary[]> {
  return invoke<ConnectedGitlabRepoSummary[]>("gitlab_list_bindings", { root });
}

export async function gitlabRefreshRepo(
  root: string,
  bindingId: string,
): Promise<ConnectedGitlabRepoSummary> {
  return invoke<ConnectedGitlabRepoSummary>("gitlab_refresh_repo", { root, bindingId });
}

export async function gitlabDisconnectRepo(root: string, bindingId: string): Promise<void> {
  await invoke("gitlab_disconnect_repo", { root, bindingId });
}

export async function gitlabListCheckoutTree(
  root: string,
  bindingId: string,
): Promise<CheckoutEntry[]> {
  return invoke<CheckoutEntry[]>("gitlab_list_checkout_tree", { root, bindingId });
}

export async function gitlabReadCheckoutFile(
  root: string,
  bindingId: string,
  relPath: string,
): Promise<CheckoutFile> {
  return invoke<CheckoutFile>("gitlab_read_checkout_file", {
    root,
    bindingId,
    relPath,
  });
}
