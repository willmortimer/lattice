import type { UIMessage } from "ai";
import { afterEach, describe, expect, it, vi } from "vitest";

import {
  assistantMessageIdForRun,
  buildAssistantContentFromChunks,
  buildAssistantPartsFromChunks,
  compactConsecutiveTextDeltas,
  findLastUserMessage,
  isApprovalPauseChunk,
  isMessageBoundaryChunk,
  isToolCompletionChunk,
  mergeTextDeltaChunk,
  persistAgentRunTranscript,
  persistIncrementalAgentRunTranscript,
  RunTranscriptAccumulator,
  shouldFlushTranscriptOnAgentEvent,
  shouldPersistAssistantMessage,
  TRANSCRIPT_PERSIST_CHUNK_BATCH,
  TranscriptPersistenceBatcher,
} from "./agentTranscriptPersistence";

const ensureMock = vi.fn();
const appendMock = vi.fn();

vi.mock("../lib/agentThreads", () => ({
  ensureAgentThread: (...args: unknown[]) => ensureMock(...args),
  appendAgentThreadMessage: (...args: unknown[]) => appendMock(...args),
}));

describe("findLastUserMessage", () => {
  it("returns the last user turn", () => {
    const messages = [
      { id: "u1", role: "user", parts: [{ type: "text", text: "first" }] },
      { id: "a1", role: "assistant", parts: [{ type: "text", text: "reply" }] },
      { id: "u2", role: "user", parts: [{ type: "text", text: "second" }] },
    ] as UIMessage[];

    expect(findLastUserMessage(messages)?.id).toBe("u2");
  });
});

describe("mergeTextDeltaChunk", () => {
  it("merges consecutive text deltas with the same id", () => {
    let chunks: unknown[] = [];
    chunks = mergeTextDeltaChunk(chunks, { type: "text-delta", id: "c1", delta: "Hello" });
    chunks = mergeTextDeltaChunk(chunks, { type: "text-delta", id: "c1", delta: " world" });

    expect(chunks).toEqual([{ type: "text-delta", id: "c1", delta: "Hello world" }]);
  });

  it("keeps separate ids as separate compacted chunks", () => {
    let chunks: unknown[] = [];
    chunks = mergeTextDeltaChunk(chunks, { type: "text-delta", id: "c1", delta: "A" });
    chunks = mergeTextDeltaChunk(chunks, { type: "text-delta", id: "c2", delta: "B" });

    expect(chunks).toEqual([
      { type: "text-delta", id: "c1", delta: "A" },
      { type: "text-delta", id: "c2", delta: "B" },
    ]);
  });
});

describe("compactConsecutiveTextDeltas", () => {
  it("folds a long delta stream into one chunk per id", () => {
    const input = [
      { type: "text-delta", id: "c1", delta: "one" },
      { type: "text-delta", id: "c1", delta: " two" },
      { type: "text-delta", id: "c1", delta: " three" },
      { type: "tool-input-available", toolCallId: "t1", toolName: "search", input: {} },
      { type: "text-delta", id: "c2", delta: "after" },
      { type: "text-delta", id: "c2", delta: " tool" },
    ];

    expect(compactConsecutiveTextDeltas(input)).toEqual([
      { type: "text-delta", id: "c1", delta: "one two three" },
      { type: "tool-input-available", toolCallId: "t1", toolName: "search", input: {} },
      { type: "text-delta", id: "c2", delta: "after tool" },
    ]);
  });
});

describe("transcript flush heuristics", () => {
  it("detects tool completion, approval pauses, and message boundaries", () => {
    expect(isToolCompletionChunk({ type: "tool-output-available", toolCallId: "t1" })).toBe(
      true,
    );
    expect(isApprovalPauseChunk({
      type: "tool-input-available",
      toolCallId: "t1",
      toolName: "request_approval",
      input: {},
    })).toBe(true);
    expect(isMessageBoundaryChunk({ type: "text-end" })).toBe(true);
    expect(shouldFlushTranscriptOnAgentEvent({ type: "approval_requested" })).toBe(true);
  });
});

describe("buildAssistantPartsFromChunks", () => {
  it("merges text deltas and preserves tool chunks", () => {
    const parts = buildAssistantPartsFromChunks([
      { type: "text-delta", id: "c1", delta: "Hello" },
      { type: "text-delta", id: "c1", delta: " world" },
      {
        type: "tool-input-available",
        toolCallId: "t1",
        toolName: "search",
        input: { q: "docs" },
      },
    ]);

    expect(parts).toEqual([
      { type: "text", text: "Hello world", id: "c1" },
      {
        type: "tool-input-available",
        toolCallId: "t1",
        toolName: "search",
        input: { q: "docs" },
      },
    ]);
  });
});

describe("buildAssistantContentFromChunks", () => {
  it("includes run errors on failed turns", () => {
    expect(
      buildAssistantContentFromChunks([], "Request contains an invalid argument."),
    ).toEqual({
      type: "uiMessage",
      role: "assistant",
      parts: [],
      error: "Request contains an invalid argument.",
    });
  });
});

describe("shouldPersistAssistantMessage", () => {
  it("persists failed runs even without streamed text", () => {
    expect(shouldPersistAssistantMessage([], "boom")).toBe(true);
    expect(shouldPersistAssistantMessage([], undefined)).toBe(false);
  });
});

describe("RunTranscriptAccumulator", () => {
  it("compacts chunks and tracks raw chunk count", () => {
    const accumulator = new RunTranscriptAccumulator();
    accumulator.observe({
      kind: "uiChunk",
      runId: "run-1",
      chunk: { type: "text-delta", id: "c1", delta: "Hi" },
    });
    accumulator.observe({
      kind: "uiChunk",
      runId: "run-1",
      chunk: { type: "text-delta", id: "c1", delta: " there" },
    });
    accumulator.observe({
      kind: "error",
      runId: "run-1",
      message: "stream failed",
    });

    expect(accumulator.rawChunkCount()).toBe(2);
    expect(accumulator.snapshot()).toEqual({
      chunks: [{ type: "text-delta", id: "c1", delta: "Hi there" }],
      runId: "run-1",
      streamError: "stream failed",
    });
  });
});

describe("TranscriptPersistenceBatcher", () => {
  afterEach(() => {
    vi.useRealTimers();
    ensureMock.mockReset();
    appendMock.mockReset();
  });

  it("flushes on chunk batch size", async () => {
    vi.useFakeTimers();
    ensureMock.mockResolvedValue(undefined);
    appendMock.mockResolvedValue(undefined);

    const accumulator = new RunTranscriptAccumulator();
    const batcher = new TranscriptPersistenceBatcher({
      workspaceRoot: "/tmp/workspace",
      threadId: "thread-1",
      messages: [
        { id: "u1", role: "user", parts: [{ type: "text", text: "hello" }] },
      ] as UIMessage[],
      accumulator,
      runId: "run-1",
      chunkBatchSize: 3,
      debounceMs: 10_000,
    });
    batcher.persistUserTurn();

    for (let index = 0; index < 3; index += 1) {
      batcher.observe({
        kind: "uiChunk",
        runId: "run-1",
        chunk: { type: "text-delta", id: "c1", delta: "x" },
      });
    }

    await vi.waitFor(() => {
      expect(appendMock.mock.calls.length).toBeGreaterThan(1);
    });
  });

  it("flushes immediately on tool completion chunks", async () => {
    ensureMock.mockResolvedValue(undefined);
    appendMock.mockResolvedValue(undefined);

    const accumulator = new RunTranscriptAccumulator();
    const batcher = new TranscriptPersistenceBatcher({
      workspaceRoot: "/tmp/workspace",
      threadId: "thread-1",
      messages: [
        { id: "u1", role: "user", parts: [{ type: "text", text: "hello" }] },
      ] as UIMessage[],
      accumulator,
      runId: "run-1",
      debounceMs: 10_000,
    });
    batcher.persistUserTurn();
    batcher.observe({
      kind: "uiChunk",
      runId: "run-1",
      chunk: { type: "text-delta", id: "c1", delta: "partial" },
    });
    batcher.observe({
      kind: "uiChunk",
      runId: "run-1",
      chunk: { type: "tool-output-available", toolCallId: "t1", output: { ok: true } },
    });

    await vi.waitFor(() => {
      expect(appendMock).toHaveBeenCalledWith(
        expect.objectContaining({
          role: "assistant",
          messageId: assistantMessageIdForRun("run-1"),
        }),
      );
    });
  });
});

describe("persistIncrementalAgentRunTranscript", () => {
  it("upserts assistant turns with a stable run-scoped id", async () => {
    ensureMock.mockReset();
    appendMock.mockReset();
    ensureMock.mockResolvedValue(undefined);
    appendMock.mockResolvedValue(undefined);

    await persistIncrementalAgentRunTranscript({
      workspaceRoot: "/tmp/workspace",
      threadId: "thread-1",
      runId: "run-1",
      messages: [
        {
          id: "u1",
          role: "user",
          parts: [{ type: "text", text: "hello" }],
        },
      ] as UIMessage[],
      chunks: [{ type: "text-delta", id: "c1", delta: "there" }],
    });

    expect(appendMock).toHaveBeenCalledTimes(2);
    expect(appendMock.mock.calls[1]?.[0]).toMatchObject({
      role: "assistant",
      runId: "run-1",
      messageId: "assistant-run-1",
      content: {
        type: "uiMessage",
        role: "assistant",
        parts: [{ type: "text", text: "there", id: "c1" }],
      },
    });
  });

  it("skips re-appending the user turn when already persisted", async () => {
    ensureMock.mockReset();
    appendMock.mockReset();
    ensureMock.mockResolvedValue(undefined);
    appendMock.mockResolvedValue(undefined);

    await persistIncrementalAgentRunTranscript({
      workspaceRoot: "/tmp/workspace",
      threadId: "thread-1",
      runId: "run-1",
      userPersisted: true,
      messages: [
        {
          id: "u1",
          role: "user",
          parts: [{ type: "text", text: "hello" }],
        },
      ] as UIMessage[],
      chunks: [{ type: "text-delta", id: "c1", delta: "more" }],
    });

    expect(appendMock).toHaveBeenCalledTimes(1);
    expect(appendMock.mock.calls[0]?.[0]).toMatchObject({ role: "assistant" });
  });
});

describe("persistAgentRunTranscript", () => {
  it("ensures the thread then appends user and assistant messages", async () => {
    ensureMock.mockReset();
    appendMock.mockReset();
    ensureMock.mockResolvedValue(undefined);
    appendMock.mockResolvedValue(undefined);

    await persistAgentRunTranscript({
      workspaceRoot: "/tmp/workspace",
      threadId: "thread-1",
      runId: "run-1",
      messages: [
        {
          id: "u1",
          role: "user",
          parts: [{ type: "text", text: "hello" }],
        },
      ] as UIMessage[],
      chunks: [{ type: "text-delta", id: "c1", delta: "there" }],
    });

    expect(ensureMock).toHaveBeenCalledWith({
      workspaceRoot: "/tmp/workspace",
      threadId: "thread-1",
    });
    expect(appendMock).toHaveBeenCalledTimes(2);
    expect(appendMock.mock.calls[0]?.[0]).toMatchObject({
      role: "user",
      runId: "run-1",
      messageId: "u1",
    });
    expect(appendMock.mock.calls[1]?.[0]).toMatchObject({
      role: "assistant",
      runId: "run-1",
      content: {
        type: "uiMessage",
        role: "assistant",
        parts: [{ type: "text", text: "there", id: "c1" }],
      },
    });
  });
});
