import type { SemanticStatus, SemanticUiEvent } from "../lib/semantic";

/** Merge daemon semantic events into cached status (monotonic download progress). */
export function mergeSemanticStatusFromEvent(
  prev: SemanticStatus | undefined,
  event: SemanticUiEvent,
): SemanticStatus {
  const nextPercent = event.progressPercent ?? null;
  const progressPercent =
    event.state === "downloading" &&
    prev?.state === "downloading" &&
    prev.progressPercent != null &&
    nextPercent != null
      ? Math.max(prev.progressPercent, nextPercent)
      : nextPercent;

  return {
    state: event.state,
    pendingChunks: event.pendingChunks,
    message: event.message,
    progressPercent,
    providerId: event.providerId ?? prev?.providerId ?? null,
    modelId: event.modelId ?? prev?.modelId ?? null,
    dimensions: event.dimensions ?? prev?.dimensions ?? null,
  };
}

/** Keep download progress monotonic when polling catches up with events. */
export function mergeSemanticStatusPoll(
  prev: SemanticStatus | undefined,
  next: SemanticStatus,
): SemanticStatus {
  if (
    next.state === "downloading" &&
    prev?.state === "downloading" &&
    prev.progressPercent != null &&
    next.progressPercent != null
  ) {
    return {
      ...next,
      progressPercent: Math.max(prev.progressPercent, next.progressPercent),
    };
  }
  return next;
}

export function isSemanticStatusActive(state: string | undefined): boolean {
  return state === "downloading" || state === "preparing" || state === "indexing";
}
