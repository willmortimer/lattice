import type { SaveState } from "../editor/saveState";
import { isUnsaved } from "../editor/saveState";
import type { TransactionProposalSummary } from "./executionContracts";
import type { AuthorityMode } from "./resourceStat";

export type ResourceTreeAuthorityBadge = "cloud" | "external" | "immutable";

export type ResourceTreeBadgeKind =
  | "dirty"
  | "proposal"
  | "agent"
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
  if (input.authorityByPath && Object.keys(input.authorityByPath).length > 0) {
    hints.authorityByPath = input.authorityByPath;
  }
  return hints;
}
