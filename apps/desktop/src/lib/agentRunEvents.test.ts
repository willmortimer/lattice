import { describe, expect, it } from "vitest";

import {
  applyRunEventsIdempotent,
  isTerminalAgentRunStatus,
  runEventToStreamMessages,
  type AgentRunEventDto,
} from "./agentRunEvents";

function event(
  partial: Partial<AgentRunEventDto> &
    Pick<AgentRunEventDto, "id" | "eventSequence" | "eventType">,
): AgentRunEventDto {
  return {
    runId: "run-1",
    threadId: "thread-1",
    payload: {},
    createdAt: 1,
    ...partial,
  };
}

describe("runEventToStreamMessages", () => {
  it("maps text and tool chunks from message_chunk payloads", () => {
    const text = runEventToStreamMessages(
      event({
        id: "e1",
        eventSequence: 1,
        eventType: "message_chunk",
        payload: {
          type: "message_chunk",
          runId: "run-1",
          chunk: { type: "text-delta", id: "c1", delta: "Hello" },
        },
      }),
    );
    expect(text.terminal).toBe(false);
    expect(text.messages).toEqual([
      {
        kind: "agentEvent",
        runId: "run-1",
        event: {
          type: "message_chunk",
          runId: "run-1",
          chunk: { type: "text-delta", id: "c1", delta: "Hello" },
        },
      },
      {
        kind: "uiChunk",
        runId: "run-1",
        chunk: { type: "text-delta", id: "c1", delta: "Hello" },
      },
    ]);

    const tool = runEventToStreamMessages(
      event({
        id: "e2",
        eventSequence: 2,
        eventType: "message_chunk",
        payload: {
          type: "message_chunk",
          runId: "run-1",
          chunk: {
            type: "tool-input-available",
            toolCallId: "t1",
            toolName: "create_proposal",
            input: { title: "Rename" },
          },
        },
      }),
    );
    expect(tool.messages.some((message) => message.kind === "uiChunk")).toBe(true);
    expect(JSON.stringify(tool.messages)).toContain("create_proposal");
  });

  it("marks terminal run events", () => {
    expect(
      runEventToStreamMessages(
        event({ id: "done", eventSequence: 3, eventType: "run_completed" }),
      ).terminal,
    ).toBe(true);
    expect(
      runEventToStreamMessages(
        event({
          id: "fail",
          eventSequence: 4,
          eventType: "run_failed",
          payload: { type: "run_failed", runId: "run-1", message: "boom", retryable: false },
        }),
      ).messages.at(-1),
    ).toEqual({ kind: "error", runId: "run-1", message: "boom" });
  });
});

describe("applyRunEventsIdempotent", () => {
  it("skips already-acked sequences and duplicate ids", () => {
    const chunks: unknown[] = [];
    const events: unknown[] = [];
    const state = { afterSequence: 1, seenIds: new Set<string>(["e1"]) };
    const result = applyRunEventsIdempotent(
      [
        event({
          id: "e1",
          eventSequence: 1,
          eventType: "message_chunk",
          payload: {
            type: "message_chunk",
            chunk: { type: "text-delta", id: "c0", delta: "skip" },
          },
        }),
        event({
          id: "e2",
          eventSequence: 2,
          eventType: "message_chunk",
          payload: {
            type: "message_chunk",
            chunk: { type: "text-delta", id: "c1", delta: "Hello" },
          },
        }),
        event({
          id: "e2",
          eventSequence: 2,
          eventType: "message_chunk",
          payload: {
            type: "message_chunk",
            chunk: { type: "text-delta", id: "c1", delta: "Hello" },
          },
        }),
        event({ id: "e3", eventSequence: 3, eventType: "run_completed" }),
      ],
      state,
      (chunk) => {
        chunks.push(chunk);
      },
      (eventValue) => {
        events.push(eventValue);
      },
    );

    expect(chunks).toEqual([{ type: "text-delta", id: "c1", delta: "Hello" }]);
    expect(result.afterSequence).toBe(3);
    expect(result.terminal).toBe(true);
    expect(isTerminalAgentRunStatus("completed")).toBe(true);
    expect(isTerminalAgentRunStatus("running")).toBe(false);
  });
});
