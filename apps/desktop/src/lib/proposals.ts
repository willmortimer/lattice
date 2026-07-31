import { invoke } from "@tauri-apps/api/core";

import type {
  CommandPreview,
  CommandPreviewDetail,
  HydrationInputDigest,
  ProposalPreview,
  ProposalSource,
  ProposalSourceType,
  ProposalStatus,
  TransactionProposal,
  TransactionProposalSummary,
} from "./executionContracts";

export type {
  CommandPreview,
  CommandPreviewDetail,
  HydrationInputDigest,
  ProposalPreview,
  ProposalSource,
  ProposalSourceType,
  ProposalStatus,
  TransactionProposal,
  TransactionProposalSummary,
} from "./executionContracts";

/** Compact hash label for provenance lists (first 8 hex chars). */
export function shortContentHash(hash: string): string {
  const trimmed = hash.trim();
  if (trimmed.length <= 8) return trimmed;
  return `${trimmed.slice(0, 8)}…`;
}

export function hasHydrationProvenance(source: ProposalSource): boolean {
  return (source.hydrationInputs?.length ?? 0) > 0;
}

export function hydrationProvenanceLabel(input: HydrationInputDigest): string {
  return `${input.path} · ${shortContentHash(input.contentHash)}`;
}

export type ProposalInboxStatusFilter = "all" | ProposalStatus;
export type ProposalInboxSourceFilter = "all" | ProposalSourceType;

export interface ProposalInboxFilters {
  status: ProposalInboxStatusFilter;
  source: ProposalInboxSourceFilter;
  pathQuery: string;
}

export interface ApplyProposalResult {
  transactionId: string;
  openPaths: string[];
}

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

/** Paths introduced or touched by the selected command subset (for open-result). */
export function pathsFromSelectedCommands(
  proposal: TransactionProposal,
  selectedCommandIndices: readonly number[],
): string[] {
  const paths = new Set<string>();
  for (const index of selectedCommandIndices) {
    const command = proposal.commands[index];
    if (!command || typeof command !== "object") continue;
    const record = command as Record<string, unknown>;
    if (typeof record.path === "string") paths.add(record.path);
    if (typeof record.to === "string") paths.add(record.to);
    if (typeof record.from === "string" && typeof record.to !== "string") {
      paths.add(record.from);
    }
    if (typeof record.toDir === "string" && typeof record.from === "string") {
      const fileName = record.from.split("/").pop();
      if (fileName) paths.add(`${record.toDir}/${fileName}`);
    }
  }
  if (paths.size > 0) return [...paths];
  return proposal.affectedPaths.slice(0, 3);
}

export function proposalStatusLabel(status: ProposalStatus): string {
  switch (status) {
    case "pending":
      return "Pending";
    case "accepted":
      return "Accepted";
    case "rejected":
      return "Rejected";
    default: {
      const _exhaustive: never = status;
      return _exhaustive;
    }
  }
}

export function filterProposalSummaries(
  proposals: readonly TransactionProposalSummary[],
  filters: ProposalInboxFilters,
): TransactionProposalSummary[] {
  const query = filters.pathQuery.trim().toLowerCase();
  return proposals.filter((item) => {
    if (filters.status !== "all" && item.status !== filters.status) return false;
    if (filters.source !== "all" && item.source.type !== filters.source) return false;
    if (!query) return true;
    if (item.summary.toLowerCase().includes(query)) return true;
    return item.affectedPaths.some((path) => path.toLowerCase().includes(query));
  });
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

export type DetailExcerptDisplay =
  | { mode: "plain"; text: string }
  | { mode: "diff"; before: string | null; after: string };

/** Compact unified `-`/`+` hunk for text-diff previews. */
export function formatTextDiffExcerpt(
  beforeExcerpt: string | undefined,
  afterExcerpt: string,
): string {
  const before = beforeExcerpt?.trim();
  if (!before) return afterExcerpt;
  if (before === afterExcerpt) return afterExcerpt;

  const lines: string[] = [];
  for (const line of before.split("\n")) {
    lines.push(`- ${line}`);
  }
  for (const line of afterExcerpt.split("\n")) {
    lines.push(`+ ${line}`);
  }
  return lines.join("\n");
}

export function detailExcerptDisplay(
  detail: CommandPreviewDetail | undefined,
): DetailExcerptDisplay | null {
  if (!detail) return null;
  switch (detail.kind) {
    case "text-create":
      return { mode: "plain", text: detail.contentExcerpt };
    case "text-diff": {
      const before = detail.beforeExcerpt?.trim() ? detail.beforeExcerpt : null;
      return { mode: "diff", before, after: detail.afterExcerpt };
    }
    case "workflow-summary":
    case "interface-summary":
    case "artifact-summary":
      return { mode: "plain", text: detail.excerpt };
    case "record-change":
      return detail.fieldSummary ? { mode: "plain", text: detail.fieldSummary } : null;
    case "file-op":
      return { mode: "plain", text: detail.paths.join(" → ") };
    default: {
      const _exhaustive: never = detail;
      return _exhaustive;
    }
  }
}

export function detailExcerpt(detail: CommandPreviewDetail | undefined): string | null {
  const display = detailExcerptDisplay(detail);
  if (!display) return null;
  if (display.mode === "plain") return display.text;
  return formatTextDiffExcerpt(display.before ?? undefined, display.after);
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
): Promise<string> {
  return invoke("apply_proposal_cmd", {
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
