/** Durable active-run reference for `reconnectToStream` after WebView reload. */

export const ACTIVE_RUN_STORAGE_KEY = "lattice.agent.activeRun.v1";

export type ActiveAgentRunRef = {
  workspaceRoot: string;
  threadId: string;
  runId: string;
  /** Client ack cursor: replay events with `eventSequence > afterSequence`. */
  afterSequence: number;
};

function storage(): Storage | null {
  if (typeof sessionStorage === "undefined") {
    return null;
  }
  return sessionStorage;
}

export function activeRunStorageKey(workspaceRoot: string, threadId: string): string {
  return `${ACTIVE_RUN_STORAGE_KEY}:${workspaceRoot}:${threadId}`;
}

/** Persist the active run id (and optional ack cursor) for a thread. */
export function persistActiveAgentRun(ref: ActiveAgentRunRef): void {
  const store = storage();
  if (!store) {
    return;
  }
  try {
    store.setItem(
      activeRunStorageKey(ref.workspaceRoot, ref.threadId),
      JSON.stringify({
        workspaceRoot: ref.workspaceRoot,
        threadId: ref.threadId,
        runId: ref.runId,
        afterSequence: Math.max(0, Math.floor(ref.afterSequence)),
      }),
    );
  } catch {
    // Ignore quota / private-mode failures.
  }
}

/** Load a previously persisted active run for this workspace + thread. */
export function loadActiveAgentRun(
  workspaceRoot: string,
  threadId: string,
): ActiveAgentRunRef | null {
  const store = storage();
  if (!store) {
    return null;
  }
  try {
    const raw = store.getItem(activeRunStorageKey(workspaceRoot, threadId));
    if (!raw) {
      return null;
    }
    const parsed = JSON.parse(raw) as Partial<ActiveAgentRunRef>;
    if (
      typeof parsed.runId !== "string" ||
      !parsed.runId.trim() ||
      typeof parsed.threadId !== "string" ||
      typeof parsed.workspaceRoot !== "string"
    ) {
      return null;
    }
    const afterSequence =
      typeof parsed.afterSequence === "number" && Number.isFinite(parsed.afterSequence)
        ? Math.max(0, Math.floor(parsed.afterSequence))
        : 0;
    return {
      workspaceRoot: parsed.workspaceRoot,
      threadId: parsed.threadId,
      runId: parsed.runId,
      afterSequence,
    };
  } catch {
    return null;
  }
}

/** Drop the active-run reference once the run is terminal (or abandoned). */
export function clearActiveAgentRun(workspaceRoot: string, threadId: string): void {
  const store = storage();
  if (!store) {
    return;
  }
  try {
    store.removeItem(activeRunStorageKey(workspaceRoot, threadId));
  } catch {
    // Ignore storage failures.
  }
}

/** Advance the ack cursor without clearing the active run. */
export function updateActiveAgentRunSequence(
  workspaceRoot: string,
  threadId: string,
  afterSequence: number,
): void {
  const existing = loadActiveAgentRun(workspaceRoot, threadId);
  if (!existing) {
    return;
  }
  if (afterSequence <= existing.afterSequence) {
    return;
  }
  persistActiveAgentRun({ ...existing, afterSequence });
}
