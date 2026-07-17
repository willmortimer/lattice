import { invoke } from "@tauri-apps/api/core";

export type LinkRepairStatus = "resolved" | "ambiguous" | "skipped";
export type LinkRepairSource = "lattice-rename" | "external-rename";
export type MarkdownLinkKind = "wiki" | "md";

export interface LinkOccurrence {
  sourcePath: string;
  kind: MarkdownLinkKind;
  rawTarget: string;
  anchor: string | null;
  label: string | null;
  sourceStartByte: number;
  sourceEndByte: number;
  sourceStartLine: number;
  sourceStartColumn: number;
  sourceEndLine: number;
  sourceEndColumn: number;
}

export interface ResourceLinkTarget {
  canonical: string;
  display: string;
  path: string;
  kind: string;
}

export interface LinkRepairCandidate {
  id: string;
  occurrence: LinkOccurrence;
  oldTarget: string;
  newTarget: string;
  newText: string;
  status: LinkRepairStatus;
  ambiguity: ResourceLinkTarget[] | null;
}

export interface LinkRepairPlan {
  id: string;
  renameFrom: string;
  renameTo: string;
  source: LinkRepairSource;
  candidates: LinkRepairCandidate[];
  createdAt: number;
}

export interface LinkRepairProposalSummary {
  id: string;
  renameFrom: string;
  renameTo: string;
  source: LinkRepairSource;
  candidateCount: number;
  unresolvedCount: number;
  createdAt: number;
}

export async function previewLinkRepair(
  root: string,
  from: string,
  to: string,
  source: LinkRepairSource = "lattice-rename",
): Promise<LinkRepairPlan> {
  return invoke("preview_link_repair", { root, from, to, source });
}

export async function getLinkRepairProposal(
  root: string,
  proposalId: string,
): Promise<LinkRepairPlan> {
  return invoke("get_link_repair_proposal", { root, proposalId });
}

export async function listLinkRepairProposals(
  root: string,
): Promise<LinkRepairProposalSummary[]> {
  return invoke("list_link_repair_proposals_cmd", { root });
}

export async function dismissLinkRepairProposal(
  root: string,
  proposalId: string,
): Promise<void> {
  await invoke("dismiss_link_repair_proposal_cmd", { root, proposalId });
}

export async function deferLinkRepairProposal(
  root: string,
  plan: LinkRepairPlan,
): Promise<void> {
  await invoke("defer_link_repair_proposal", { root, plan });
}

export async function applyLinkRepair(
  root: string,
  from: string,
  to: string,
  acceptedCandidateIds: string[],
  plan: LinkRepairPlan,
): Promise<void> {
  await invoke("apply_link_repair", {
    root,
    from,
    to,
    acceptedCandidateIds,
    plan,
  });
}

export async function applyLinkRepairProposal(
  root: string,
  proposalId: string,
  acceptedCandidateIds: string[],
): Promise<void> {
  await invoke("apply_link_repair_proposal", {
    root,
    proposalId,
    acceptedCandidateIds,
  });
}

export function defaultAcceptedCandidateIds(plan: LinkRepairPlan): string[] {
  return plan.candidates
    .filter((candidate) => candidate.status === "resolved")
    .map((candidate) => candidate.id);
}
