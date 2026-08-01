import { Channel } from "@tauri-apps/api/core";
import type { UIMessageChunk } from "ai";

import type { AgentStreamMsg } from "./agent";
import { invoke } from "./ipc";

export type AgentRunStatus = "running" | "completed" | "failed" | "cancelled";

export type AgentRunStatusDto = {
  runId: string;
  threadId: string;
  status: AgentRunStatus;
  lastSequence: number;
  createdAt: number;
  updatedAt: number;
};

export type AgentRunEventDto = {
  id: string;
  runId: string;
  threadId: string;
  eventSequence: number;
  eventType: string;
  payload: unknown;
  createdAt: number;
};

export type GetAgentRunStatusArgs = {
  workspaceRoot: string;
  runId?: string;
  threadId?: string;
};

export type GetAgentRunStatusResult = {
  workspaceId: string;
  run: AgentRunStatusDto | null;
};

export type ListAgentRunEventsArgs = {
  workspaceRoot: string;
  runId: string;
  afterSequence?: number;
};

export type ListAgentRunEventsResult = {
  workspaceId: string;
  runId: string;
  afterSequence: number;
  events: AgentRunEventDto[];
  run: AgentRunStatusDto;
};

export type SubscribeAgentRunArgs = {
  workspaceRoot: string;
  runId: string;
  afterSequence?: number;
};

export type SubscribeAgentRunResult = {
  runId: string;
  threadId: string;
  lastSequence: number;
  status: AgentRunStatus;
  /** Present when the run ended in failure. */
  error?: string;
};

/** Fetch run status by run id, or the active run for a thread. */
export async function getAgentRunStatus(
  args: GetAgentRunStatusArgs,
): Promise<GetAgentRunStatusResult> {
  return invoke<GetAgentRunStatusResult>("agent_run_status", {
    args: {
      workspaceRoot: args.workspaceRoot,
      runId: args.runId,
      threadId: args.threadId,
    },
  });
}

/** List durable run events with `eventSequence > afterSequence`. */
export async function listAgentRunEvents(
  args: ListAgentRunEventsArgs,
): Promise<ListAgentRunEventsResult> {
  return invoke<ListAgentRunEventsResult>("agent_run_list_events", {
    args: {
      workspaceRoot: args.workspaceRoot,
      runId: args.runId,
      afterSequence: args.afterSequence ?? 0,
    },
  });
}

/**
 * Replay missed events then live-tail until the run is terminal.
 * Delivers the same Channel shapes as `agent_start_run`.
 */
export async function subscribeAgentRun(
  args: SubscribeAgentRunArgs,
  onMessage: (message: AgentStreamMsg) => void,
): Promise<SubscribeAgentRunResult> {
  const channel = new Channel<AgentStreamMsg>((message) => {
    onMessage(message);
  });
  return invoke<SubscribeAgentRunResult>("agent_subscribe_run", {
    args: {
      workspaceRoot: args.workspaceRoot,
      runId: args.runId,
      afterSequence: args.afterSequence ?? 0,
    },
    channel,
  });
}

/** True when a durable run status will not emit further events. */
export function isTerminalAgentRunStatus(status: AgentRunStatus): boolean {
  return status !== "running";
}

/**
 * Map one durable run-event row into Channel messages (mirrors Tauri
 * `agent_event_messages`). Proposal/tool chunks ride inside `message_chunk`
 * payloads alongside text deltas.
 */
export function runEventToStreamMessages(event: AgentRunEventDto): {
  messages: AgentStreamMsg[];
  terminal: boolean;
} {
  const runId = event.runId;
  const payload =
    event.payload && typeof event.payload === "object"
      ? (event.payload as Record<string, unknown>)
      : {};
  const messages: AgentStreamMsg[] = [
    {
      kind: "agentEvent",
      runId,
      event: event.payload,
    },
  ];

  if (event.eventType === "message_chunk" && "chunk" in payload) {
    messages.push({
      kind: "uiChunk",
      runId,
      chunk: payload.chunk,
    });
  }

  switch (event.eventType) {
    case "run_completed":
    case "run_cancelled":
      messages.push({ kind: "done", runId });
      return { messages, terminal: true };
    case "run_failed": {
      const message =
        typeof payload.message === "string" && payload.message.trim()
          ? payload.message
          : "agent run failed";
      messages.push({ kind: "error", runId, message });
      return { messages, terminal: true };
    }
    default:
      return { messages, terminal: false };
  }
}

/**
 * Apply ordered run events idempotently: skips sequences already seen, advances
 * the cursor monotonically, and extracts UI chunks (text + tool/proposal).
 */
export function applyRunEventsIdempotent(
  events: AgentRunEventDto[],
  state: { afterSequence: number; seenIds: Set<string> },
  enqueue: (chunk: UIMessageChunk) => void,
  onAgentEvent?: (event: unknown) => void,
): { afterSequence: number; terminal: boolean; terminalError?: string } {
  let afterSequence = state.afterSequence;
  let terminal = false;
  let terminalError: string | undefined;

  for (const event of events) {
    if (event.eventSequence <= afterSequence) {
      continue;
    }
    if (state.seenIds.has(event.id)) {
      afterSequence = Math.max(afterSequence, event.eventSequence);
      continue;
    }
    state.seenIds.add(event.id);

    const mapped = runEventToStreamMessages(event);
    for (const message of mapped.messages) {
      switch (message.kind) {
        case "uiChunk":
          enqueue(message.chunk as UIMessageChunk);
          break;
        case "agentEvent":
          onAgentEvent?.(message.event);
          break;
        case "done":
          terminal = true;
          break;
        case "error":
          terminal = true;
          terminalError = message.message;
          break;
        default: {
          const _exhaustive: never = message;
          return _exhaustive;
        }
      }
    }
    afterSequence = event.eventSequence;
    if (mapped.terminal) {
      terminal = true;
      break;
    }
  }

  return { afterSequence, terminal, terminalError };
}
