import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  applyAgentStreamMessage,
  type AgentStreamMsg,
  TauriAgentChatTransport,
} from "./agent";

const invokeMock = vi.fn();
const channelInstances: Array<{ onmessage: (message: AgentStreamMsg) => void }> = [];

vi.mock("@tauri-apps/api/core", () => ({
  Channel: vi.fn(function ChannelMock(
    this: { onmessage: (message: AgentStreamMsg) => void },
    onmessage: (message: AgentStreamMsg) => void,
  ) {
    this.onmessage = onmessage;
    channelInstances.push(this);
  }),
}));

vi.mock("./ipc", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

describe("applyAgentStreamMessage", () => {
  it("maps uiChunk payloads into the AI SDK stream", () => {
    const chunks: unknown[] = [];
    const outcome = applyAgentStreamMessage(
      {
        kind: "uiChunk",
        runId: "run-1",
        chunk: { type: "text-delta", id: "c1", delta: "Hello" },
      },
      (chunk) => {
        chunks.push(chunk);
      },
      {},
    );

    expect(outcome).toBe("continue");
    expect(chunks).toEqual([{ type: "text-delta", id: "c1", delta: "Hello" }]);
  });

  it("forwards agent events without secrets", () => {
    const events: unknown[] = [];
    const outcome = applyAgentStreamMessage(
      {
        kind: "agentEvent",
        runId: "run-1",
        event: {
          type: "run_started",
          runId: "run-1",
          threadId: "thread-1",
        },
      },
      () => {},
      {
        onAgentEvent: (event) => {
          events.push(event);
        },
        onRunId: (runId) => {
          events.push({ trackedRunId: runId });
        },
      },
    );

    expect(outcome).toBe("continue");
    expect(events).toEqual([
      { type: "run_started", runId: "run-1", threadId: "thread-1" },
      { trackedRunId: "run-1" },
    ]);
    expect(JSON.stringify(events)).not.toContain("apiKey");
    expect(JSON.stringify(events)).not.toContain("PIONEER");
  });
});

describe("TauriAgentChatTransport", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    channelInstances.length = 0;
  });

  it("assembles a fake chunk sequence from channel callbacks", async () => {
    invokeMock.mockImplementation(async (_command, payload) => {
      const channel = payload.channel as { onmessage: (message: AgentStreamMsg) => void };
      channel.onmessage({
        kind: "agentEvent",
        runId: "run-1",
        event: { type: "run_started", runId: "run-1", threadId: "thread-1" },
      });
      channel.onmessage({
        kind: "uiChunk",
        runId: "run-1",
        chunk: { type: "text-delta", id: "c1", delta: "Hello" },
      });
      channel.onmessage({
        kind: "uiChunk",
        runId: "run-1",
        chunk: { type: "text-delta", id: "c2", delta: " world" },
      });
      channel.onmessage({ kind: "done", runId: "run-1" });
      return { runId: "run-1", threadId: "thread-1" };
    });

    const transport = new TauriAgentChatTransport({
      workspaceRoot: "/tmp/workspace",
      threadId: "thread-1",
    });

    const stream = await transport.sendMessages({
      chatId: "thread-1",
      trigger: "submit-message",
      messageId: undefined,
      messages: [
        {
          id: "m1",
          role: "user",
          parts: [{ type: "text", text: "hi" }],
        },
      ],
      abortSignal: undefined,
    });

    const reader = stream.getReader();
    const chunks: unknown[] = [];
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      chunks.push(value);
    }

    expect(invokeMock).toHaveBeenCalledWith(
      "agent_start_run",
      expect.objectContaining({
        args: expect.objectContaining({
          workspaceRoot: "/tmp/workspace",
          threadId: "thread-1",
          messagesJson: expect.stringContaining('"role":"user"'),
        }),
      }),
    );
    expect(chunks).toEqual([
      { type: "text-delta", id: "c1", delta: "Hello" },
      { type: "text-delta", id: "c2", delta: " world" },
    ]);
    expect(JSON.stringify(chunks)).not.toContain("apiKey");
  });

  it("cancels the active run via stop()", async () => {
    invokeMock.mockImplementation(async (command, payload) => {
      if (command === "agent_cancel_run") {
        return undefined;
      }
      const channel = payload.channel as { onmessage: (message: AgentStreamMsg) => void };
      channel.onmessage({
        kind: "agentEvent",
        runId: "run-9",
        event: { type: "run_started", runId: "run-9", threadId: "thread-1" },
      });
      return new Promise(() => {});
    });

    const transport = new TauriAgentChatTransport({
      workspaceRoot: "/tmp/workspace",
      threadId: "thread-1",
    });

    void transport.sendMessages({
      chatId: "thread-1",
      trigger: "submit-message",
      messageId: undefined,
      messages: [
        {
          id: "m1",
          role: "user",
          parts: [{ type: "text", text: "hi" }],
        },
      ],
      abortSignal: undefined,
    });

    await vi.waitFor(() => {
      expect(transport.getActiveRunId()).toBe("run-9");
    });

    await transport.stop();
    expect(invokeMock).toHaveBeenCalledWith("agent_cancel_run", { runId: "run-9" });
  });
});
