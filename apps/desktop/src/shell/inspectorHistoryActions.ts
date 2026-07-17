import {
  getResourceRevision,
  listResourceRevisions,
  revertResourceRevision,
  type ResourceRevisionDetail,
  type ResourceRevisionSummary,
} from "../lib/revisions";

export async function loadResourceHistory(
  root: string,
  path: string,
  limit = 50,
): Promise<ResourceRevisionSummary[]> {
  return listResourceRevisions(root, path, limit);
}

export async function loadResourceHistoryDetail(
  root: string,
  path: string,
  revisionId: string,
): Promise<ResourceRevisionDetail | null> {
  return getResourceRevision(root, path, revisionId);
}

/** Revert only when a current baseline revision is known (session or list marker). */
export async function guardedRevertResourceRevision(args: {
  root: string;
  path: string;
  revisionId: string;
  expectedCurrentRevision: string | null;
}): Promise<string> {
  if (!args.expectedCurrentRevision) {
    throw new Error("Cannot revert: current revision is unknown.");
  }
  return revertResourceRevision(
    args.root,
    args.path,
    args.revisionId,
    args.expectedCurrentRevision,
  );
}

export function resolveExpectedCurrentRevision(
  revisions: ResourceRevisionSummary[],
  sessionCurrentRevision: string | null,
): string | null {
  if (sessionCurrentRevision) return sessionCurrentRevision;
  const baseline = revisions.find((item) => item.currentBaseline);
  return baseline?.revisionId ?? null;
}

export function formatRevisionDiff(detail: ResourceRevisionDetail): string {
  if (detail.diff.unified) return detail.diff.unified;
  if (!detail.summary.priorAvailable) return "Prior content unavailable.";
  if (detail.diff.isBinary) return "Binary revision — unified diff unavailable.";
  return "Prior content unavailable.";
}
