import type { UIMessage } from "ai";
import { describe, expect, it, vi } from "vitest";

import {
  buildAssistantContentFromChunks,
  buildAssistantPartsFromChunks,
  findLastUserMessage,
  persistAgentRunTranscript,
  RunTranscriptAccumulator,
  shouldPersistAssistantMessage,
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
  it("collects chunks and stream errors", () => {
    const accumulator = new RunTranscriptAccumulator();
    accumulator.observe({
      kind: "uiChunk",
      runId: "run-1",
      chunk: { type: "text-delta", id: "c1", delta: "Hi" },
    });
    accumulator.observe({
      kind: "error",
      runId: "run-1",
      message: "stream failed",
    });

    expect(accumulator.snapshot()).toEqual({
      chunks: [{ type: "text-delta", id: "c1", delta: "Hi" }],
      runId: "run-1",
      streamError: "stream failed",
    });
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
