import type { UIMessage } from "ai";

import type { AgentStreamMsg } from "../lib/agent";
import { appendAgentThreadMessage, ensureAgentThread } from "../lib/agentThreads";

export type MessagePart = Record<string, unknown>;

export type PersistAgentRunTranscriptArgs = {
  workspaceRoot: string;
  threadId: string;
  messages: UIMessage[];
  chunks: unknown[];
  runId: string;
  error?: string;
};

/** Last user turn submitted for this run. */
export function findLastUserMessage(messages: UIMessage[]): UIMessage | null {
  for (let index = messages.length - 1; index >= 0; index -= 1) {
    const message = messages[index];
    if (message?.role === "user") {
      return message;
    }
  }
  return null;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function chunkType(chunk: unknown): string | null {
  if (!isRecord(chunk) || typeof chunk.type !== "string") {
    return null;
  }
  return chunk.type;
}

/** Build assistant-ui parts from streamed AI SDK chunks. */
export function buildAssistantPartsFromChunks(chunks: unknown[]): MessagePart[] {
  const parts: MessagePart[] = [];
  let textBuffer = "";
  let textId: string | undefined;

  const flushText = () => {
    if (!textBuffer) {
      return;
    }
    parts.push({
      type: "text",
      text: textBuffer,
      ...(textId ? { id: textId } : {}),
    });
    textBuffer = "";
    textId = undefined;
  };

  for (const chunk of chunks) {
    const type = chunkType(chunk);
    if (type === "text-delta") {
      const record = chunk as Record<string, unknown>;
      if (typeof record.id === "string" && !textId) {
        textId = record.id;
      }
      if (typeof record.delta === "string") {
        textBuffer += record.delta;
      }
      continue;
    }

    flushText();

    if (
      type === "tool-input-available" ||
      type === "tool-output-available" ||
      type === "tool-call" ||
      type === "tool-result"
    ) {
      parts.push(chunk as MessagePart);
    }
  }

  flushText();
  return parts;
}

export function buildAssistantContentFromChunks(
  chunks: unknown[],
  error?: string,
): Record<string, unknown> | null {
  const parts = buildAssistantPartsFromChunks(chunks);
  if (parts.length === 0 && !error) {
    return null;
  }
  const content: Record<string, unknown> = {
    type: "uiMessage",
    role: "assistant",
    parts,
  };
  if (error) {
    content.error = error;
  }
  return content;
}

export function buildUserMessageContent(message: UIMessage): Record<string, unknown> {
  return {
    type: "uiMessage",
    ...message,
  };
}

export function shouldPersistAssistantMessage(
  chunks: unknown[],
  error?: string,
): boolean {
  return Boolean(error) || buildAssistantPartsFromChunks(chunks).length > 0;
}

/** Ensure the thread exists, then append the user and assistant turns for one run. */
export async function persistAgentRunTranscript(
  args: PersistAgentRunTranscriptArgs,
): Promise<void> {
  const userMessage = findLastUserMessage(args.messages);
  const assistantContent = buildAssistantContentFromChunks(args.chunks, args.error);

  if (!userMessage && !assistantContent) {
    return;
  }

  await ensureAgentThread({
    workspaceRoot: args.workspaceRoot,
    threadId: args.threadId,
  });

  if (userMessage) {
    await appendAgentThreadMessage({
      workspaceRoot: args.workspaceRoot,
      threadId: args.threadId,
      role: "user",
      content: buildUserMessageContent(userMessage),
      runId: args.runId,
      messageId: userMessage.id,
    });
  }

  if (assistantContent) {
    await appendAgentThreadMessage({
      workspaceRoot: args.workspaceRoot,
      threadId: args.threadId,
      role: "assistant",
      content: assistantContent,
      runId: args.runId,
    });
  }
}

/** Collect streamed chunks and terminal errors for one agent run. */
export class RunTranscriptAccumulator {
  private chunks: unknown[] = [];
  private runId: string | null = null;
  private streamError: string | null = null;

  observe(message: AgentStreamMsg): void {
    if (!this.runId) {
      this.runId = message.runId;
    }
    switch (message.kind) {
      case "uiChunk":
        this.chunks.push(message.chunk);
        break;
      case "error":
        this.streamError = message.message;
        break;
      default:
        break;
    }
  }

  snapshot(): { chunks: unknown[]; runId: string | null; streamError: string | null } {
    return {
      chunks: [...this.chunks],
      runId: this.runId,
      streamError: this.streamError,
    };
  }
}
