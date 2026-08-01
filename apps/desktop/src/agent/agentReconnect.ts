import { loadActiveAgentRun } from "../lib/agentActiveRun";

/** True when sessionStorage holds an in-flight run for this workspace thread. */
export function hasPersistedActiveAgentRun(
  workspaceRoot: string,
  threadId: string,
): boolean {
  return loadActiveAgentRun(workspaceRoot, threadId) !== null;
}

/**
 * Resume a persisted active run via AI SDK `resumeStream` (transport.reconnectToStream).
 * Returns whether a reconnect was attempted.
 */
export async function reconnectPersistedActiveAgentRun(
  workspaceRoot: string,
  threadId: string,
  resumeStream: () => Promise<void>,
): Promise<boolean> {
  if (!hasPersistedActiveAgentRun(workspaceRoot, threadId)) {
    return false;
  }
  await resumeStream();
  return true;
}
