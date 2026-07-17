import { invoke } from "@tauri-apps/api/core";

export type RevisionSource = "local" | "external";

export interface ResourceRevisionSummary {
  revisionId: string;
  resourcePath: string;
  transactionId: string | null;
  summary: string | null;
  createdAt: number;
  parentRevision: string | null;
  beforeHash: string | null;
  afterHash: string | null;
  beforeLen: number | null;
  afterLen: number | null;
  source: RevisionSource;
  priorAvailable: boolean;
  pinned: boolean;
  currentBaseline: boolean;
  unresolvedConflict: boolean;
}

export interface RevisionPayload {
  hash: string;
  len: number;
  isText: boolean;
  text: string | null;
}

export interface RevisionDiff {
  isBinary: boolean;
  unified: string | null;
  addedLines: number;
  removedLines: number;
  baseLen: number | null;
  localLen: number | null;
}

export interface ConflictEnvelope {
  resource: string;
  baseRevision: string | null;
  incompatibleDescendants: string[];
  affectedUnits: string[];
  failureReason: string;
  resolutionOptions: string[];
}

export interface ResourceRevisionDetail {
  summary: ResourceRevisionSummary;
  base: RevisionPayload | null;
  local: RevisionPayload | null;
  incoming: RevisionPayload | null;
  diff: RevisionDiff;
  conflict: ConflictEnvelope | null;
}

export function listResourceRevisions(
  root: string,
  path: string,
  limit = 50,
): Promise<ResourceRevisionSummary[]> {
  return invoke("list_resource_revisions", { root, relPath: path, limit });
}

export function getResourceRevision(
  root: string,
  path: string,
  revisionId: string,
): Promise<ResourceRevisionDetail | null> {
  return invoke("get_resource_revision", { root, relPath: path, revisionId });
}

export function revertResourceRevision(
  root: string,
  path: string,
  revisionId: string,
  expectedCurrentRevision: string,
): Promise<string> {
  return invoke("revert_resource_revision", {
    root,
    relPath: path,
    revisionId,
    expectedCurrentRevision,
  });
}

export interface HistoryCleanupCandidate {
  objectHash: string;
  size: number;
  createdAt: number;
}

export interface HistoryCleanupReport {
  dryRun: boolean;
  requiresConfirmation: boolean;
  notice: string | null;
  totalBytes: number;
  reclaimableBytes: number;
  candidates: HistoryCleanupCandidate[];
  deletedObjects: number;
  deletedBytes: number;
}

export function cleanupHistory(args: {
  root: string;
  dryRun: boolean;
  maxAgeDays?: number;
  maxBytes?: number;
}): Promise<HistoryCleanupReport> {
  return invoke("cleanup_history", {
    root: args.root,
    dryRun: args.dryRun,
    maxAgeDays: args.maxAgeDays ?? null,
    maxBytes: args.maxBytes ?? null,
  });
}
