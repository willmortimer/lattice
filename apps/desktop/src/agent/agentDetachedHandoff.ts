import {
  clearActiveAgentRun,
  loadActiveAgentRun,
  persistActiveAgentRun,
  type ActiveAgentRunRef,
} from "../lib/agentActiveRun";

/** Shared across webviews (sessionStorage is not). */
export const AGENT_DETACHED_HANDOFF_KEY = "lattice.agent.detached.handoff.v1";

export type AgentDetachedReturnLayout = "dock" | "workbench" | "focus";

export type AgentDetachedHandoff = {
  workspaceRoot: string;
  threadId: string;
  returnLayoutMode: AgentDetachedReturnLayout;
  /** Copied so the detached window can own live-tail reconnect. */
  activeRun: ActiveAgentRunRef | null;
};

function localStore(): Storage | null {
  if (typeof localStorage === "undefined") {
    return null;
  }
  return localStorage;
}

export function isAgentDetachedReturnLayout(value: unknown): value is AgentDetachedReturnLayout {
  return value === "dock" || value === "workbench" || value === "focus";
}

export function parseAgentDetachedHandoff(raw: string | null): AgentDetachedHandoff | null {
  if (!raw) {
    return null;
  }
  try {
    const parsed = JSON.parse(raw) as Partial<AgentDetachedHandoff>;
    if (
      typeof parsed.workspaceRoot !== "string" ||
      !parsed.workspaceRoot.trim() ||
      typeof parsed.threadId !== "string" ||
      !parsed.threadId.trim() ||
      !isAgentDetachedReturnLayout(parsed.returnLayoutMode)
    ) {
      return null;
    }
    let activeRun: ActiveAgentRunRef | null = null;
    if (parsed.activeRun && typeof parsed.activeRun === "object") {
      const run = parsed.activeRun;
      if (
        typeof run.runId === "string" &&
        run.runId.trim() &&
        typeof run.threadId === "string" &&
        typeof run.workspaceRoot === "string" &&
        typeof run.afterSequence === "number" &&
        Number.isFinite(run.afterSequence)
      ) {
        activeRun = {
          workspaceRoot: run.workspaceRoot,
          threadId: run.threadId,
          runId: run.runId,
          afterSequence: Math.max(0, Math.floor(run.afterSequence)),
        };
      }
    }
    return {
      workspaceRoot: parsed.workspaceRoot.trim(),
      threadId: parsed.threadId.trim(),
      returnLayoutMode: parsed.returnLayoutMode,
      activeRun,
    };
  } catch {
    return null;
  }
}

export function readAgentDetachedHandoff(): AgentDetachedHandoff | null {
  const store = localStore();
  if (!store) {
    return null;
  }
  return parseAgentDetachedHandoff(store.getItem(AGENT_DETACHED_HANDOFF_KEY));
}

export function writeAgentDetachedHandoff(handoff: AgentDetachedHandoff): void {
  const store = localStore();
  if (!store) {
    return;
  }
  try {
    store.setItem(AGENT_DETACHED_HANDOFF_KEY, JSON.stringify(handoff));
  } catch {
    // Ignore quota / private-mode failures.
  }
}

export function clearAgentDetachedHandoff(): void {
  const store = localStore();
  if (!store) {
    return;
  }
  try {
    store.removeItem(AGENT_DETACHED_HANDOFF_KEY);
  } catch {
    // Ignore storage failures.
  }
}

/** Build handoff from the main window's current thread + optional in-flight run. */
export function buildAgentDetachedHandoff(input: {
  workspaceRoot: string;
  threadId: string;
  returnLayoutMode: AgentDetachedReturnLayout;
}): AgentDetachedHandoff {
  const workspaceRoot = input.workspaceRoot.trim();
  const threadId = input.threadId.trim();
  return {
    workspaceRoot,
    threadId,
    returnLayoutMode: input.returnLayoutMode,
    activeRun: loadActiveAgentRun(workspaceRoot, threadId),
  };
}

/**
 * Seed this webview's sessionStorage so LatticeAgentProvider can reconnect,
 * then clear the shared handoff active-run copy to avoid stale dual ownership.
 */
export function applyAgentDetachedHandoffToSession(handoff: AgentDetachedHandoff): void {
  if (handoff.activeRun) {
    persistActiveAgentRun(handoff.activeRun);
  } else {
    clearActiveAgentRun(handoff.workspaceRoot, handoff.threadId);
  }
}

/** Capture this webview's active run into shared handoff before yielding ownership. */
export function refreshAgentDetachedHandoffActiveRun(
  handoff: AgentDetachedHandoff,
): AgentDetachedHandoff {
  const next: AgentDetachedHandoff = {
    ...handoff,
    activeRun: loadActiveAgentRun(handoff.workspaceRoot, handoff.threadId),
  };
  writeAgentDetachedHandoff(next);
  return next;
}
