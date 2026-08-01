import type { UIMessage } from "ai";

import type { AgentStreamMsg } from "../lib/agent";
import { appendAgentThreadMessage, ensureAgentThread } from "../lib/agentThreads";

export type MessagePart = Record<string, unknown>;

export const TRANSCRIPT_PERSIST_CHUNK_BATCH = 50;
export const TRANSCRIPT_PERSIST_DEBOUNCE_MS = 375;

export type PersistAgentRunTranscriptArgs = {
  workspaceRoot: string;
  threadId: string;
  messages: UIMessage[];
  chunks: unknown[];
  runId: string;
  error?: string;
};

export type PersistIncrementalAgentRunTranscriptArgs = PersistAgentRunTranscriptArgs & {
  userPersisted?: boolean;
  assistantMessageId?: string;
};

export type PersistIncrementalAgentRunTranscriptResult = {
  userPersisted: boolean;
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

function textDeltaId(chunk: unknown): string | null {
  if (!isRecord(chunk) || chunk.type !== "text-delta") {
    return null;
  }
  return typeof chunk.id === "string" ? chunk.id : null;
}

function textDeltaDelta(chunk: unknown): string {
  if (!isRecord(chunk) || chunk.type !== "text-delta") {
    return "";
  }
  return typeof chunk.delta === "string" ? chunk.delta : "";
}

/** Merge one incoming text-delta into a compacted chunk list. */
export function mergeTextDeltaChunk(chunks: unknown[], chunk: unknown): unknown[] {
  const type = chunkType(chunk);
  if (type !== "text-delta") {
    return [...chunks, chunk];
  }

  const incomingId = textDeltaId(chunk);
  const incomingDelta = textDeltaDelta(chunk);
  if (!incomingDelta) {
    return chunks;
  }

  const last = chunks.at(-1);
  const lastType = chunkType(last);
  if (lastType === "text-delta" && textDeltaId(last) === incomingId) {
    const merged = [...chunks];
    const lastRecord = { ...(last as Record<string, unknown>) };
    lastRecord.delta = `${textDeltaDelta(last)}${incomingDelta}`;
    if (incomingId && !lastRecord.id) {
      lastRecord.id = incomingId;
    }
    merged[merged.length - 1] = lastRecord;
    return merged;
  }

  return [...chunks, chunk];
}

/** Fold an entire chunk list so consecutive text-deltas share one entry per id. */
export function compactConsecutiveTextDeltas(chunks: unknown[]): unknown[] {
  let compacted: unknown[] = [];
  for (const chunk of chunks) {
    compacted = mergeTextDeltaChunk(compacted, chunk);
  }
  return compacted;
}

const TOOL_COMPLETION_CHUNK_TYPES = new Set(["tool-output-available", "tool-result"]);

/** Flush persistence when a tool finishes streaming output. */
export function isToolCompletionChunk(chunk: unknown): boolean {
  const type = chunkType(chunk);
  return type !== null && TOOL_COMPLETION_CHUNK_TYPES.has(type);
}

const APPROVAL_TOOL_NAMES = new Set([
  "approval",
  "request_approval",
  "confirm_action",
]);

/** Flush persistence when an approval tool pauses the run for human input. */
export function isApprovalPauseChunk(chunk: unknown): boolean {
  if (!isRecord(chunk)) {
    return false;
  }
  const type = chunkType(chunk);
  if (type === "tool-input-available" || type === "tool-call") {
    const toolName =
      typeof chunk.toolName === "string" ? chunk.toolName.trim().toLowerCase() : "";
    return APPROVAL_TOOL_NAMES.has(toolName);
  }
  return false;
}

const MESSAGE_BOUNDARY_CHUNK_TYPES = new Set([
  "text-start",
  "text-end",
  "start",
  "finish",
  "message-metadata",
]);

/** Flush persistence on AI SDK message boundaries. */
export function isMessageBoundaryChunk(chunk: unknown): boolean {
  const type = chunkType(chunk);
  return type !== null && MESSAGE_BOUNDARY_CHUNK_TYPES.has(type);
}

/** Flush persistence when daemon agent events signal approval pauses. */
export function shouldFlushTranscriptOnAgentEvent(event: unknown): boolean {
  if (!isRecord(event) || typeof event.type !== "string") {
    return false;
  }
  const type = event.type.trim().toLowerCase();
  return (
    type === "approval_requested" ||
    type === "human_approval_required" ||
    type === "approval_required" ||
    type === "run_paused"
  );
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

  for (const chunk of compactConsecutiveTextDeltas(chunks)) {
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

export function assistantMessageIdForRun(runId: string): string {
  return `assistant-${runId}`;
}

/**
 * Persist the current user turn once and upsert the in-progress assistant turn.
 * Daemon run-event logs remain authoritative for raw stream replay (A0).
 */
export async function persistIncrementalAgentRunTranscript(
  args: PersistIncrementalAgentRunTranscriptArgs,
): Promise<PersistIncrementalAgentRunTranscriptResult> {
  const userMessage = findLastUserMessage(args.messages);
  const assistantContent = buildAssistantContentFromChunks(args.chunks, args.error);
  let userPersisted = args.userPersisted ?? false;

  if (!userMessage && !assistantContent) {
    return { userPersisted };
  }

  await ensureAgentThread({
    workspaceRoot: args.workspaceRoot,
    threadId: args.threadId,
  });

  if (userMessage && !userPersisted) {
    await appendAgentThreadMessage({
      workspaceRoot: args.workspaceRoot,
      threadId: args.threadId,
      role: "user",
      content: buildUserMessageContent(userMessage),
      runId: args.runId,
      messageId: userMessage.id,
    });
    userPersisted = true;
  }

  if (assistantContent) {
    await appendAgentThreadMessage({
      workspaceRoot: args.workspaceRoot,
      threadId: args.threadId,
      role: "assistant",
      content: assistantContent,
      runId: args.runId,
      messageId: args.assistantMessageId ?? assistantMessageIdForRun(args.runId),
    });
  }

  return { userPersisted };
}

/** Ensure the thread exists, then append the user and assistant turns for one run. */
export async function persistAgentRunTranscript(
  args: PersistAgentRunTranscriptArgs,
): Promise<void> {
  await persistIncrementalAgentRunTranscript(args);
}

export type TranscriptPersistenceBatcherOptions = {
  workspaceRoot: string;
  threadId: string;
  messages: UIMessage[];
  accumulator: RunTranscriptAccumulator;
  runId: string;
  /**
   * When true (default), raw stream events are already durable in the daemon
   * run-event log; this batcher only maintains compacted thread transcript rows.
   */
  daemonAuthoritativeRunEvents?: boolean;
  debounceMs?: number;
  chunkBatchSize?: number;
};

/** Batch thread transcript persistence during an active agent run. */
export class TranscriptPersistenceBatcher {
  private userPersisted = false;
  private lastFlushRawChunkCount = 0;
  private timer: ReturnType<typeof setTimeout> | null = null;
  private disposed = false;
  private flushInFlight = false;
  private flushQueued = false;
  private readonly debounceMs: number;
  private readonly chunkBatchSize: number;

  constructor(private readonly options: TranscriptPersistenceBatcherOptions) {
    this.debounceMs = options.debounceMs ?? TRANSCRIPT_PERSIST_DEBOUNCE_MS;
    this.chunkBatchSize = options.chunkBatchSize ?? TRANSCRIPT_PERSIST_CHUNK_BATCH;
  }

  /** Persist the submitted user turn at the run/message boundary. */
  persistUserTurn(): void {
    void this.flush({ force: true, userOnly: true });
  }

  observe(message: AgentStreamMsg): void {
    if (this.disposed) {
      return;
    }

    if (message.kind === "uiChunk") {
      this.options.accumulator.observe(message);
      const chunk = message.chunk;
      if (
        isMessageBoundaryChunk(chunk) ||
        isToolCompletionChunk(chunk) ||
        isApprovalPauseChunk(chunk)
      ) {
        void this.flush({ force: true });
        return;
      }

      const rawCount = this.options.accumulator.rawChunkCount();
      if (rawCount - this.lastFlushRawChunkCount >= this.chunkBatchSize) {
        void this.flush({ force: true });
        return;
      }

      this.scheduleDebounce();
      return;
    }

    if (message.kind === "agentEvent") {
      if (shouldFlushTranscriptOnAgentEvent(message.event)) {
        void this.flush({ force: true });
      }
      return;
    }

    if (message.kind === "error") {
      this.options.accumulator.observe(message);
      void this.flush({ force: true, final: true });
    }
  }

  /** Final flush when the transport completes or fails. */
  dispose(error?: string): void {
    if (this.disposed) {
      return;
    }
    this.disposed = true;
    this.clearTimer();
    void this.flush({ force: true, final: true, error });
  }

  private scheduleDebounce(): void {
    if (this.timer !== null) {
      return;
    }
    this.timer = setTimeout(() => {
      this.timer = null;
      void this.flush();
    }, this.debounceMs);
  }

  private clearTimer(): void {
    if (this.timer !== null) {
      clearTimeout(this.timer);
      this.timer = null;
    }
  }

  private async flush(args?: {
    force?: boolean;
    final?: boolean;
    error?: string;
    userOnly?: boolean;
  }): Promise<void> {
    if (this.disposed && !args?.final) {
      return;
    }

    if (this.flushInFlight) {
      this.flushQueued = true;
      return;
    }

    this.clearTimer();
    this.flushInFlight = true;

    try {
      const snapshot = this.options.accumulator.snapshot();
      const runId = snapshot.runId ?? this.options.runId;
      const error =
        args?.error ?? snapshot.streamError ?? undefined;
      const chunks = args?.userOnly ? [] : snapshot.chunks;

      if (
        !args?.force &&
        !args?.final &&
        !error &&
        chunks.length === 0 &&
        !findLastUserMessage(this.options.messages)
      ) {
        return;
      }

      const result = await persistIncrementalAgentRunTranscript({
        workspaceRoot: this.options.workspaceRoot,
        threadId: this.options.threadId,
        messages: this.options.messages,
        chunks,
        runId,
        error,
        userPersisted: this.userPersisted,
        assistantMessageId: assistantMessageIdForRun(runId),
      });
      this.userPersisted = result.userPersisted;
      this.lastFlushRawChunkCount = this.options.accumulator.rawChunkCount();
    } catch {
      // Persistence must not block or surface in the composer.
    } finally {
      this.flushInFlight = false;
      if (this.flushQueued) {
        this.flushQueued = false;
        void this.flush({ force: true, final: args?.final, error: args?.error });
      }
    }
  }
}

/** Collect streamed chunks and terminal errors for one agent run. */
export class RunTranscriptAccumulator {
  private chunks: unknown[] = [];
  private runId: string | null = null;
  private streamError: string | null = null;
  private observedRawChunkCount = 0;

  observe(message: AgentStreamMsg): void {
    if (!this.runId) {
      this.runId = message.runId;
    }
    switch (message.kind) {
      case "uiChunk":
        this.observedRawChunkCount += 1;
        this.chunks = mergeTextDeltaChunk(this.chunks, message.chunk);
        break;
      case "error":
        this.streamError = message.message;
        break;
      default:
        break;
    }
  }

  rawChunkCount(): number {
    return this.observedRawChunkCount;
  }

  snapshot(): { chunks: unknown[]; runId: string | null; streamError: string | null } {
    return {
      chunks: [...this.chunks],
      runId: this.runId,
      streamError: this.streamError,
    };
  }
}
