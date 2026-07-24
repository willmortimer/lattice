import { invoke } from "./ipc";

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
