import { invoke } from "./ipc";

export interface GithubOAuthStartResult {
  sessionId: string;
  authorizeUrl: string;
  redirectUri: string;
}

export interface GithubRepoSummary {
  id: number;
  full_name: string;
  owner: string;
  name: string;
  default_branch: string;
  private: boolean;
  installation_id?: number | null;
  clone_url: string;
}

export interface GithubRepoBinding {
  kind: string;
  id: string;
  owner: string;
  repo: string;
  repo_id: number;
  default_branch: string;
  head_sha?: string | null;
  installation_id?: number | null;
  mode: "read";
  credentials: { provider: string; key: string };
  extract: { strategy: string; depth: number; path: string };
  capabilities: string[];
  last_refreshed_at?: string | null;
  stale?: boolean | null;
  last_error?: string | null;
}

export interface ConnectedRepoSummary {
  binding: GithubRepoBinding;
  checkout_exists: boolean;
  stale: boolean;
}

export interface CheckoutEntry {
  path: string;
  is_dir: boolean;
  size?: number | null;
}

export interface CheckoutFile {
  path: string;
  content: string;
  byte_len: number;
}

export async function githubOauthBegin(): Promise<GithubOAuthStartResult> {
  return invoke<GithubOAuthStartResult>("github_oauth_begin");
}

/** Blocks until the browser completes the loopback redirect (up to ~5 minutes). */
export async function githubOauthFinish(sessionId: string): Promise<string> {
  return invoke<string>("github_oauth_finish", { sessionId });
}

export async function githubListRepos(accessToken: string): Promise<GithubRepoSummary[]> {
  return invoke<GithubRepoSummary[]>("github_list_repos", { accessToken });
}

export async function githubConnectRepo(input: {
  root: string;
  accessToken: string;
  owner: string;
  repo: string;
  repoId: number;
  defaultBranch: string;
  installationId?: number | null;
}): Promise<ConnectedRepoSummary> {
  return invoke<ConnectedRepoSummary>("github_connect_repo", {
    root: input.root,
    accessToken: input.accessToken,
    owner: input.owner,
    repo: input.repo,
    repoId: input.repoId,
    defaultBranch: input.defaultBranch,
    installationId: input.installationId ?? null,
  });
}

export async function githubListBindings(root: string): Promise<ConnectedRepoSummary[]> {
  return invoke<ConnectedRepoSummary[]>("github_list_bindings", { root });
}

export async function githubRefreshRepo(
  root: string,
  bindingId: string,
): Promise<ConnectedRepoSummary> {
  return invoke<ConnectedRepoSummary>("github_refresh_repo", { root, bindingId });
}

export async function githubDisconnectRepo(root: string, bindingId: string): Promise<void> {
  await invoke("github_disconnect_repo", { root, bindingId });
}

export async function githubListCheckoutTree(
  root: string,
  bindingId: string,
): Promise<CheckoutEntry[]> {
  return invoke<CheckoutEntry[]>("github_list_checkout_tree", { root, bindingId });
}

export async function githubReadCheckoutFile(
  root: string,
  bindingId: string,
  relPath: string,
): Promise<CheckoutFile> {
  return invoke<CheckoutFile>("github_read_checkout_file", {
    root,
    bindingId,
    relPath,
  });
}
