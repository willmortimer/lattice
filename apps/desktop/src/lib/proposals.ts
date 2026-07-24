import { invoke } from "@tauri-apps/api/core";

import type {
  CommandPreview,
  CommandPreviewDetail,
  ProposalPreview,
  ProposalSourceType,
  TransactionProposal,
  TransactionProposalSummary,
} from "./executionContracts";

export type {
  CommandPreview,
  CommandPreviewDetail,
  ProposalPreview,
  ProposalSource,
  ProposalSourceType,
  ProposalStatus,
  TransactionProposal,
  TransactionProposalSummary,
} from "./executionContracts";

export interface CreateProposalInput {
  summary: string;
  commands: unknown[];
  affectedPaths?: string[];
  warnings?: string[];
  sourceType?: ProposalSourceType;
  sourceResource?: string;
}

/** Default selection: every command index in order. */
export function defaultAcceptedCommandIndices(proposal: TransactionProposal): number[] {
  return proposal.commands.map((_, index) => index);
}

/** Human-readable label for a serialized command in the review list. */
export function commandSummaryLabel(command: unknown, index: number): string {
  if (!command || typeof command !== "object") {
    return `Command ${index + 1}`;
  }
  const record = command as Record<string, unknown>;
  const type = typeof record.type === "string" ? record.type : `command-${index + 1}`;
  const path =
    typeof record.path === "string"
      ? record.path
      : typeof record.from === "string"
        ? record.from
        : null;
  return path ? `${type}: ${path}` : type;
}

/** Prefer backend preview summary; fall back to local command labeling. */
export function previewCommandLabel(
  preview: CommandPreview | undefined,
  command: unknown,
  index: number,
): string {
  if (preview?.summary) return preview.summary;
  return commandSummaryLabel(command, index);
}

export function detailExcerpt(detail: CommandPreviewDetail | undefined): string | null {
  if (!detail) return null;
  switch (detail.kind) {
    case "text-create":
      return detail.contentExcerpt;
    case "text-diff":
      return detail.afterExcerpt;
    case "workflow-summary":
    case "interface-summary":
    case "artifact-summary":
      return detail.excerpt;
    case "record-change":
      return detail.fieldSummary || null;
    case "file-op":
      return detail.paths.join(" → ");
    default: {
      const _exhaustive: never = detail;
      return _exhaustive;
    }
  }
}

export async function createProposal(
  root: string,
  proposal: CreateProposalInput,
): Promise<TransactionProposal> {
  return invoke("create_proposal_cmd", { root, proposal });
}

export async function getProposal(
  root: string,
  proposalId: string,
): Promise<TransactionProposal> {
  return invoke("get_proposal", { root, proposalId });
}

export async function listProposals(root: string): Promise<TransactionProposalSummary[]> {
  return invoke("list_proposals", { root });
}

export async function dismissProposal(root: string, proposalId: string): Promise<void> {
  await invoke("dismiss_proposal_cmd", { root, proposalId });
}

export async function applyProposal(
  root: string,
  proposalId: string,
  selectedCommandIndices: number[],
): Promise<void> {
  await invoke("apply_proposal_cmd", {
    root,
    proposalId,
    selectedCommandIndices,
  });
}

export async function previewProposal(
  root: string,
  proposalId: string,
  selectedCommandIndices: number[],
): Promise<ProposalPreview> {
  return invoke("preview_proposal_cmd", {
    root,
    proposalId,
    selectedCommandIndices,
  });
}

export async function validateProposalSubset(
  root: string,
  proposalId: string,
  selectedCommandIndices: number[],
): Promise<void> {
  await invoke("validate_proposal_subset_cmd", {
    root,
    proposalId,
    selectedCommandIndices,
  });
}

export async function createDemoProposal(root: string): Promise<TransactionProposal> {
  return invoke("create_demo_proposal", { root });
}
