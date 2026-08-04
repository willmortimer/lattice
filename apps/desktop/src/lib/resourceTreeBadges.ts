import type { SaveState } from "../editor/saveState";
import { isUnsaved } from "../editor/saveState";
import type { TransactionProposalSummary } from "./executionContracts";
import type {
  ExecuteOutcome,
  PlannerSyncStatus,
  WorkspaceSyncExecuteResult,
  WorkspaceSyncRunReport,
} from "./cloudSync";
import type { AuthorityMode } from "./resourceStat";
import type { CatalogEntry } from "./resourceCatalog";
import { pathForResourceId } from "./resourceCatalog";

export type ResourceTreeAuthorityBadge = "cloud" | "external" | "immutable";

export type ResourceTreeSyncBadge = "syncConflict" | "syncError";

export type ResourceTreeBadgeKind =
  | "dirty"
  | "proposal"
  | "agent"
  | ResourceTreeSyncBadge
  | ResourceTreeAuthorityBadge;

export interface ResourceTreeRowBadge {
  kind: ResourceTreeBadgeKind;
  label: string;
  title: string;
}

/** Minimal path/id maps passed into `ResourceTree` from the shell. */
export interface ResourceTreeBadgeHints {
  dirtyByPath?: ReadonlySet<string>;
  proposalByPath?: ReadonlySet<string>;
  agentByResourceId?: ReadonlySet<string>;
  agentByPath?: ReadonlySet<string>;
  syncByPath?: Readonly<Record<string, ResourceTreeSyncBadge>>;
  authorityByPath?: Readonly<Record<string, ResourceTreeAuthorityBadge>>;
}

export interface ResourceTreeBadgeRowInput {
  resourceId: string;
  path: string;
  hints?: ResourceTreeBadgeHints;
}

const AGENT_PROPOSAL_SOURCES = new Set<TransactionProposalSummary["source"]["type"]>([
  "mcp",
  "external",
  "task",
  "workflow",
]);

const AUTHORITY_BADGE_LABELS: Record<ResourceTreeAuthorityBadge, string> = {
  cloud: "Cloud",
  external: "Ext",
  immutable: "Lock",
};

const SYNC_BADGE_LABELS: Record<ResourceTreeSyncBadge, string> = {
  syncConflict: "Conflict",
  syncError: "Sync",
};

const SYNC_BADGE_TITLES: Record<ResourceTreeSyncBadge, string> = {
  syncConflict: "Sync conflict — cloud and local versions disagree; nothing was overwritten",
  syncError: "Cloud sync failed for this resource",
};

export function syncBadgeForExecuteResult(
  result: Pick<WorkspaceSyncExecuteResult, "status" | "outcome">,
): ResourceTreeSyncBadge | null {
  if (result.outcome === "skipped_conflicted" || result.status === "conflicted") {
    return "syncConflict";
  }
  if (result.outcome === "failed") {
    return "syncError";
  }
  return null;
}

export function syncBadgesByPathFromReport(
  report: WorkspaceSyncRunReport,
  catalog: ReadonlyMap<string, CatalogEntry>,
): Record<string, ResourceTreeSyncBadge> {
  const badges: Record<string, ResourceTreeSyncBadge> = {};
  for (const result of report.results) {
    const badge = syncBadgeForExecuteResult(result);
    if (!badge) continue;
    const path = pathForResourceId(catalog, result.resourceId);
    if (!path) continue;
    badges[path] = badge;
  }
  return badges;
}

/** Test helper for planner status / outcome pairs. */
export function syncBadgeForPlannerOutcome(
  status: PlannerSyncStatus,
  outcome: ExecuteOutcome,
): ResourceTreeSyncBadge | null {
  return syncBadgeForExecuteResult({ status, outcome });
}

export function authorityBadgeForMode(
  authority: AuthorityMode,
): ResourceTreeAuthorityBadge | null {
  switch (authority) {
    case "cloud":
      return "cloud";
    case "external":
      return "external";
    case "immutable_import":
      return "immutable";
    case "local":
      return null;
    default:
      return authority satisfies never;
  }
}

/** Resolve visible row badges from minimal shell hints. */
export function resourceTreeRowBadges(
  input: ResourceTreeBadgeRowInput,
): ResourceTreeRowBadge[] {
  const hints = input.hints;
  if (!hints) return [];

  const badges: ResourceTreeRowBadge[] = [];

  if (hints.dirtyByPath?.has(input.path)) {
    badges.push({
      kind: "dirty",
      label: "•",
      title: "Unsaved changes",
    });
  }

  if (hints.proposalByPath?.has(input.path)) {
    badges.push({
      kind: "proposal",
      label: "P",
      title: "Pending proposal",
    });
  }

  if (
    hints.agentByResourceId?.has(input.resourceId)
    || hints.agentByPath?.has(input.path)
  ) {
    badges.push({
      kind: "agent",
      label: "A",
      title: "Agent activity",
    });
  }

  const syncBadge = hints.syncByPath?.[input.path];
  if (syncBadge) {
    badges.push({
      kind: syncBadge,
      label: SYNC_BADGE_LABELS[syncBadge],
      title: SYNC_BADGE_TITLES[syncBadge],
    });
  }

  const authority = hints.authorityByPath?.[input.path];
  if (authority) {
    badges.push({
      kind: authority,
      label: AUTHORITY_BADGE_LABELS[authority],
      title: `Authority: ${AUTHORITY_BADGE_LABELS[authority]}`,
    });
  }

  return badges;
}

export interface BuildResourceTreeBadgeHintsInput {
  saveStatusBySessionId: Readonly<Record<string, SaveState>>;
  proposalSummaries: readonly TransactionProposalSummary[];
  agentPanelOpen?: boolean;
  selectedPath?: string | null;
  syncByPath?: Readonly<Record<string, ResourceTreeSyncBadge>>;
  authorityByPath?: Readonly<Record<string, ResourceTreeAuthorityBadge>>;
}

/** Build sidebar badge maps from controller / shell state. */
export function buildResourceTreeBadgeHints(
  input: BuildResourceTreeBadgeHintsInput,
): ResourceTreeBadgeHints {
  const dirtyByPath = new Set<string>();
  for (const [sessionId, status] of Object.entries(input.saveStatusBySessionId)) {
    if (isUnsaved(status)) dirtyByPath.add(sessionId);
  }

  const proposalByPath = new Set<string>();
  const agentByPath = new Set<string>();
  for (const proposal of input.proposalSummaries) {
    if (proposal.status !== "pending") continue;
    for (const path of proposal.affectedPaths) {
      proposalByPath.add(path);
      if (AGENT_PROPOSAL_SOURCES.has(proposal.source.type)) {
        agentByPath.add(path);
      }
    }
  }

  if (input.agentPanelOpen && input.selectedPath) {
    agentByPath.add(input.selectedPath);
  }

  const hints: ResourceTreeBadgeHints = {};
  if (dirtyByPath.size > 0) hints.dirtyByPath = dirtyByPath;
  if (proposalByPath.size > 0) hints.proposalByPath = proposalByPath;
  if (agentByPath.size > 0) hints.agentByPath = agentByPath;
  if (input.syncByPath && Object.keys(input.syncByPath).length > 0) {
    hints.syncByPath = input.syncByPath;
  }
  if (input.authorityByPath && Object.keys(input.authorityByPath).length > 0) {
    hints.authorityByPath = input.authorityByPath;
  }
  return hints;
}
