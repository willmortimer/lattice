import { invoke } from "./ipc";

export type AuthorityMode = "local" | "cloud" | "external" | "immutable_import";

export type MaterializationState =
  | "metadata_only"
  | "cached"
  | "pinned"
  | "evicted";

export interface HydrationInputDigest {
  path: string;
  contentHash: string;
  resourceId?: string | null;
}

export interface ResourceStat {
  resource_id: string;
  path: string;
  authority: AuthorityMode;
  materialization: MaterializationState;
  content_hash: string | null;
  version_id: string | null;
  hydration_inputs?: HydrationInputDigest[];
}

const AUTHORITY_LABELS: Record<AuthorityMode, string> = {
  local: "Local",
  cloud: "Cloud",
  external: "External",
  immutable_import: "Immutable import",
};

const MATERIALIZATION_LABELS: Record<MaterializationState, string> = {
  metadata_only: "Metadata only",
  cached: "Cached",
  pinned: "Pinned",
  evicted: "Evicted",
};

export function formatAuthority(mode: AuthorityMode): string {
  return AUTHORITY_LABELS[mode];
}

export function formatMaterialization(state: MaterializationState): string {
  return MATERIALIZATION_LABELS[state];
}

export function getResourceStat(root: string, relPath: string): Promise<ResourceStat> {
  return invoke<ResourceStat>("get_resource_stat", { root, relPath });
}
