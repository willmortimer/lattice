import type { UIMessage } from "ai";

import { invoke } from "./ipc";

export type EnsureAgentThreadArgs = {
  workspaceRoot: string;
  threadId: string;
  title?: string;
};

export type AppendAgentThreadMessageArgs = {
  workspaceRoot: string;
  threadId: string;
  role: string;
  content: unknown;
  runId?: string;
  messageId?: string;
};

export type ListAgentThreadsArgs = {
  workspaceRoot: string;
};

export type GetAgentThreadArgs = {
  workspaceRoot: string;
  threadId: string;
};

export type AgentThreadSummary = {
  id: string;
  title?: string | null;
  createdAt: number;
  updatedAt: number;
};

export type AgentThreadMessage = {
  id: string;
  threadId: string;
  role: string;
  content: unknown;
  runId?: string | null;
  createdAt: number;
};

export type ListAgentThreadsResult = {
  workspaceId: string;
  threads: AgentThreadSummary[];
};

export type GetAgentThreadResult = {
  workspaceId: string;
  thread: AgentThreadSummary;
  messages: AgentThreadMessage[];
};

/** Create the workspace-local thread row when missing. */
export async function ensureAgentThread(args: EnsureAgentThreadArgs): Promise<void> {
  await invoke<void>("agent_thread_ensure", {
    args: {
      workspaceRoot: args.workspaceRoot,
      threadId: args.threadId,
      title: args.title,
    },
  });
}

/** Append one durable message to a workspace-local agent thread. */
export async function appendAgentThreadMessage(
  args: AppendAgentThreadMessageArgs,
): Promise<void> {
  await invoke<void>("agent_thread_append_message", {
    args: {
      workspaceRoot: args.workspaceRoot,
      threadId: args.threadId,
      role: args.role,
      content: args.content,
      runId: args.runId,
      messageId: args.messageId,
    },
  });
}

/** List workspace-local agent threads (metadata only). */
export async function listAgentThreads(
  args: ListAgentThreadsArgs,
): Promise<ListAgentThreadsResult> {
  return invoke<ListAgentThreadsResult>("agent_thread_list", {
    args: { workspaceRoot: args.workspaceRoot },
  });
}

/** Fetch one thread and its durable messages. */
export async function getAgentThread(
  args: GetAgentThreadArgs,
): Promise<GetAgentThreadResult> {
  return invoke<GetAgentThreadResult>("agent_thread_get", {
    args: {
      workspaceRoot: args.workspaceRoot,
      threadId: args.threadId,
    },
  });
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

/** Human label for the thread selector / list. */
export function displayTitleForThread(thread: Pick<AgentThreadSummary, "id" | "title">): string {
  const title = typeof thread.title === "string" ? thread.title.trim() : "";
  if (title) {
    return title;
  }
  const shortId = thread.id.replace(/-/g, "").slice(0, 8);
  return shortId ? `Thread ${shortId}` : "Untitled thread";
}

/**
 * Map a durable thread message into an AI SDK UIMessage for resume.
 * Prefers B2 `uiMessage` content envelopes; falls back to text content.
 */
export function uiMessageFromStoredContent(
  message: Pick<AgentThreadMessage, "id" | "role" | "content">,
): UIMessage | null {
  const role =
    message.role === "user" || message.role === "assistant" || message.role === "system"
      ? message.role
      : null;
  if (!role) {
    return null;
  }

  const content = message.content;
  if (isRecord(content) && content.type === "uiMessage") {
    const parts = Array.isArray(content.parts) ? content.parts : [];
    const id =
      typeof content.id === "string" && content.id.trim() ? content.id.trim() : message.id;
    return {
      id,
      role,
      parts: parts as UIMessage["parts"],
    } as UIMessage;
  }

  if (isRecord(content) && typeof content.text === "string") {
    return {
      id: message.id,
      role,
      parts: [{ type: "text", text: content.text }],
    } as UIMessage;
  }

  if (typeof content === "string" && content.trim()) {
    return {
      id: message.id,
      role,
      parts: [{ type: "text", text: content }],
    } as UIMessage;
  }

  return {
    id: message.id,
    role,
    parts: [],
  } as UIMessage;
}

/** Convert durable thread messages into ordered UIMessages for the chat runtime. */
export function uiMessagesFromThreadMessages(messages: AgentThreadMessage[]): UIMessage[] {
  const result: UIMessage[] = [];
  for (const message of messages) {
    const ui = uiMessageFromStoredContent(message);
    if (ui) {
      result.push(ui);
    }
  }
  return result;
}
