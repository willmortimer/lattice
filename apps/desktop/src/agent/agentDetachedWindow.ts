import { emitTo } from "@tauri-apps/api/event";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";

import { clearActiveAgentRun } from "../lib/agentActiveRun";
import {
  buildAgentDetachedHandoff,
  type AgentDetachedReturnLayout,
  writeAgentDetachedHandoff,
} from "./agentDetachedHandoff";

export const AGENT_DETACHED_WINDOW_LABEL = "agent-detached";
export const AGENT_DETACHED_OPEN_EVENT = "agent-detached-open";
export const AGENT_DETACHED_CLOSE_EVENT = "agent-detached-close";
export const AGENT_DETACHED_CLOSED_EVENT = "agent-detached-closed";

export type AgentDetachedOpenPayload = {
  workspaceRoot: string;
  threadId: string;
  returnLayoutMode: AgentDetachedReturnLayout;
};

export type AgentDetachedClosedPayload = {
  returnLayoutMode: AgentDetachedReturnLayout;
  workspaceRoot: string;
  threadId: string;
};

/**
 * Show and focus the detached agent window for the same workspace thread.
 *
 * Mirrors `showQuickNote`: targets the preconfigured `agent-detached` label.
 * Writes a localStorage handoff (including any active-run cursor) so the
 * detached webview becomes the sole live-tail reconnect owner.
 */
export async function showDetachedAgent(input: {
  workspaceRoot: string;
  threadId: string;
  returnLayoutMode: AgentDetachedReturnLayout;
}): Promise<void> {
  const handoff = buildAgentDetachedHandoff(input);
  writeAgentDetachedHandoff(handoff);
  // Drop main-window session ownership so only the detached webview reconnects.
  clearActiveAgentRun(handoff.workspaceRoot, handoff.threadId);

  const window = await WebviewWindow.getByLabel(AGENT_DETACHED_WINDOW_LABEL);
  if (!window) {
    throw new Error("Detached agent window is unavailable.");
  }
  await window.show();
  await window.setFocus();
  await emitTo(AGENT_DETACHED_WINDOW_LABEL, AGENT_DETACHED_OPEN_EVENT, {
    workspaceRoot: handoff.workspaceRoot,
    threadId: handoff.threadId,
    returnLayoutMode: handoff.returnLayoutMode,
  } satisfies AgentDetachedOpenPayload);
}

/** Ask the detached window to yield reconnect ownership and hide. */
export async function requestCloseDetachedAgent(): Promise<void> {
  const window = await WebviewWindow.getByLabel(AGENT_DETACHED_WINDOW_LABEL);
  if (!window) {
    return;
  }
  await emitTo(AGENT_DETACHED_WINDOW_LABEL, AGENT_DETACHED_CLOSE_EVENT, {});
}
