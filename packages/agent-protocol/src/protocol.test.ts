import { describe, expect, it } from "vitest";

import {
  PROTOCOL_VERSION,
  parseCommand,
  parseEvent,
  serializeCommand,
  serializeEvent,
  type AgentCommand,
  type AgentEvent,
} from "./index";

describe("PROTOCOL_VERSION", () => {
  it("is 1", () => {
    expect(PROTOCOL_VERSION).toBe(1);
  });
});

describe("commands", () => {
  const commandFixtures: AgentCommand[] = [
    { type: "hello", protocolVersion: PROTOCOL_VERSION },
    {
      type: "start_run",
      threadId: "thread-1",
      runId: "run-1",
      provider: "pioneer",
      model: "gpt-test",
      messages: [{ id: "m1", role: "user", content: "hello" }],
    },
    {
      type: "start_run",
      threadId: "thread-2",
      runId: "run-2",
      provider: "openai",
      model: "gpt-4.1",
      prompt: "Summarize the workspace.",
      workspaceId: "ws-1",
      workspaceRoot: "/tmp/demo",
    },
    {
      type: "start_run",
      threadId: "thread-3",
      runId: "run-3",
      provider: "fake",
      model: "fake-model",
      messages: [{ id: "m2", role: "system", instructions: "be helpful" }],
      prompt: "ignored when messages are present",
    },
    { type: "cancel_run", runId: "run-1" },
    { type: "health" },
    { type: "shutdown" },
  ];

  it.each(commandFixtures)("round-trips %o", (command) => {
    const line = serializeCommand(command);
    expect(line).not.toMatch(/\n/);
    expect(parseCommand(line)).toEqual(command);
    expect(parseCommand(`  ${line}  `)).toEqual(command);
  });

  it("rejects malformed command lines", () => {
    expect(() => parseCommand("")).toThrow();
    expect(() => parseCommand("not json")).toThrow();
    expect(() => parseCommand('{"type":"hello"}')).toThrow();
    expect(() => parseCommand('{"type":"hello","protocolVersion":2}')).toThrow();
    expect(() =>
      parseCommand(
        JSON.stringify({
          type: "start_run",
          threadId: "t",
          runId: "r",
          provider: "pioneer",
          model: "m",
        }),
      ),
    ).toThrow();
    expect(() =>
      parseCommand(JSON.stringify({ type: "unknown", runId: "r" })),
    ).toThrow();
  });
});

describe("events", () => {
  const eventFixtures: AgentEvent[] = [
    { type: "hello_ack", protocolVersion: PROTOCOL_VERSION },
    { type: "run_started", runId: "run-1", threadId: "thread-1", provider: "pioneer" },
    {
      type: "message_chunk",
      runId: "run-1",
      chunk: { type: "text-delta", id: "m1", delta: "hi" },
    },
    { type: "run_completed", runId: "run-1" },
    {
      type: "run_failed",
      runId: "run-1",
      message: "provider unavailable",
      retryable: true,
    },
    { type: "health", ok: true },
    {
      type: "step_started",
      runId: "run-1",
      stepId: "step-nav",
      kind: "navigation",
      label: "Focus inventory row",
    },
    {
      type: "step_completed",
      runId: "run-1",
      stepId: "step-nav",
      durationMs: 42,
      summary: "Revealed table row",
    },
    {
      type: "evidence_added",
      runId: "run-1",
      evidenceId: "ev-1",
      resourceId: "page:notes",
      path: "notes.md",
      excerpt: "Quarterly revenue increased.",
      anchor: {
        kind: "markdown-block",
        resourceId: "page:notes",
        blockId: "blk-2",
      },
      score: 0.91,
    },
    {
      type: "overlay_show",
      runId: "run-1",
      overlayId: "ov-1",
      anchors: [
        {
          kind: "dataset-region",
          resourceId: "table:inventory",
          rowKeys: ["row-7"],
          columns: ["sku"],
        },
      ],
      purpose: "attention",
      commentary: "Low stock",
    },
    { type: "overlay_clear", runId: "run-1", overlayId: "ov-1" },
    { type: "overlay_clear", runId: "run-1" },
  ];

  it.each(eventFixtures)("round-trips %o", (event) => {
    const line = serializeEvent(event);
    expect(line).not.toMatch(/\n/);
    expect(parseEvent(line)).toEqual(event);
    expect(parseEvent(`  ${line}  `)).toEqual(event);
  });

  it("rejects malformed event lines", () => {
    expect(() => parseEvent("")).toThrow();
    expect(() => parseEvent("{")).toThrow();
    expect(() => parseEvent('{"type":"hello_ack"}')).toThrow();
    expect(() => parseEvent('{"type":"hello_ack","protocolVersion":0}')).toThrow();
    expect(() =>
      parseEvent(JSON.stringify({ type: "run_failed", runId: "r" })),
    ).toThrow();
    expect(() =>
      parseEvent(
        JSON.stringify({
          type: "overlay_show",
          runId: "run-1",
          overlayId: "ov-1",
          anchors: [],
          purpose: "attention",
        }),
      ),
    ).toThrow();
    expect(() =>
      parseEvent(
        JSON.stringify({
          type: "step_completed",
          runId: "run-1",
          stepId: "step-1",
          durationMs: -1,
        }),
      ),
    ).toThrow();
  });
});
