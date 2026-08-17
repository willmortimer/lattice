import type { PagePersistMode } from "../editor/collab/collabSession";
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

export type ResourceAuthority =
  | { kind: "plain_file" }
  | {
      kind: "collaborative";
      doc_id: string;
      materialized_revision?: string | null;
    };

export interface ResourceStat {
  resource_id: string;
  path: string;
  authority: AuthorityMode;
  materialization: MaterializationState;
  resource_authority?: ResourceAuthority;
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

const RESOURCE_AUTHORITY_LABELS: Record<PagePersistMode, string> = {
  plain: "Plain file",
  collaborative: "Collaborative",
};

export function formatAuthority(mode: AuthorityMode): string {
  return AUTHORITY_LABELS[mode];
}

export function formatMaterialization(state: MaterializationState): string {
  return MATERIALIZATION_LABELS[state];
}

export function formatResourceAuthority(authority: ResourceAuthority | undefined): string {
  const mode = persistModeFromResourceAuthority(authority);
  return RESOURCE_AUTHORITY_LABELS[mode];
}

export function persistModeFromResourceAuthority(
  authority: ResourceAuthority | undefined,
): PagePersistMode {
  if (!authority) {
    return "plain";
  }
  switch (authority.kind) {
    case "plain_file":
      return "plain";
    case "collaborative":
      return "collaborative";
    default: {
      const neverAuthority: never = authority;
      return neverAuthority;
    }
  }
}

export function persistModeFromResourceStat(
  stat: ResourceStat,
  registryResourceId: string | undefined,
  labsCollabOn: boolean,
): PagePersistMode {
  if (!labsCollabOn || !registryResourceId) {
    return "plain";
  }
  const authority = stat.resource_authority;
  if (!authority) {
    return "plain";
  }
  switch (authority.kind) {
    case "plain_file":
      return "plain";
    case "collaborative":
      return authority.doc_id === registryResourceId ? "collaborative" : "plain";
    default: {
      const neverAuthority: never = authority;
      return neverAuthority;
    }
  }
}

export function getResourceStat(root: string, relPath: string): Promise<ResourceStat> {
  return invoke<ResourceStat>("get_resource_stat", { root, relPath });
}

export function setResourceAuthority(
  root: string,
  relPath: string,
  authority: ResourceAuthority,
): Promise<ResourceStat> {
  return invoke<ResourceStat>("set_resource_authority", { root, relPath, authority });
}

export function resourceAuthorityForPersistMode(
  mode: PagePersistMode,
  registryResourceId: string,
): ResourceAuthority {
  switch (mode) {
    case "plain":
      return { kind: "plain_file" };
    case "collaborative":
      return {
        kind: "collaborative",
        doc_id: registryResourceId,
        materialized_revision: null,
      };
    default: {
      const neverMode: never = mode;
      return neverMode;
    }
  }
}
