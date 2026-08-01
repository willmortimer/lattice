import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  applyAgentStreamMessage,
  type AgentStreamMsg,
  TauriAgentChatTransport,
} from "./agent";
import { loadActiveAgentRun } from "./agentActiveRun";

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
  const memory = new Map<string, string>();

  beforeEach(() => {
    invokeMock.mockReset();
    channelInstances.length = 0;
    memory.clear();
    Object.defineProperty(globalThis, "sessionStorage", {
      configurable: true,
      value: {
        getItem: (key: string) => memory.get(key) ?? null,
        setItem: (key: string, value: string) => {
          memory.set(key, value);
        },
        removeItem: (key: string) => {
          memory.delete(key);
        },
      },
    });
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
      persistTranscripts: false,
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

  it("persists the active run id on start_run", async () => {
    invokeMock.mockImplementation(async (_command, payload) => {
      const channel = payload.channel as { onmessage: (message: AgentStreamMsg) => void };
      channel.onmessage({
        kind: "agentEvent",
        runId: "run-persist",
        event: { type: "run_started", runId: "run-persist", threadId: "thread-1" },
      });
      expect(loadActiveAgentRun("/tmp/workspace", "thread-1")?.runId).toBe("run-persist");
      // Leave the invoke hanging so reconnect can observe the active ref mid-run.
      return new Promise(() => {});
    });

    const transport = new TauriAgentChatTransport({
      workspaceRoot: "/tmp/workspace",
      threadId: "thread-1",
      persistTranscripts: false,
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
      expect(transport.getActiveRunId()).toBe("run-persist");
    });
    expect(loadActiveAgentRun("/tmp/workspace", "thread-1")).toEqual({
      workspaceRoot: "/tmp/workspace",
      threadId: "thread-1",
      runId: "run-persist",
      afterSequence: 0,
    });
  });

  it("reconnectToStream replays then live-tails via subscribe_run", async () => {
    memory.set(
      "lattice.agent.activeRun.v1:/tmp/workspace:thread-1",
      JSON.stringify({
        workspaceRoot: "/tmp/workspace",
        threadId: "thread-1",
        runId: "run-re",
        afterSequence: 1,
      }),
    );

    invokeMock.mockImplementation(async (command, payload) => {
      if (command === "agent_run_status") {
        return {
          workspaceId: "ws",
          run: {
            runId: "run-re",
            threadId: "thread-1",
            status: "running",
            lastSequence: 1,
            createdAt: 1,
            updatedAt: 1,
          },
        };
      }
      if (command === "agent_subscribe_run") {
        const channel = payload.channel as { onmessage: (message: AgentStreamMsg) => void };
        channel.onmessage({
          kind: "uiChunk",
          runId: "run-re",
          chunk: { type: "text-delta", id: "c2", delta: " resumed" },
        });
        channel.onmessage({
          kind: "uiChunk",
          runId: "run-re",
          chunk: {
            type: "tool-input-available",
            toolCallId: "t1",
            toolName: "create_proposal",
            input: { title: "Ship A1" },
          },
        });
        channel.onmessage({ kind: "done", runId: "run-re" });
        return {
          runId: "run-re",
          threadId: "thread-1",
          lastSequence: 4,
          status: "completed",
        };
      }
      throw new Error(`unexpected command ${String(command)}`);
    });

    const transport = new TauriAgentChatTransport({
      workspaceRoot: "/tmp/workspace",
      threadId: "thread-1",
      persistTranscripts: false,
    });

    const stream = await transport.reconnectToStream();
    expect(stream).not.toBeNull();
    const reader = stream!.getReader();
    const chunks: unknown[] = [];
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      chunks.push(value);
    }

    expect(invokeMock).toHaveBeenCalledWith(
      "agent_run_status",
      expect.objectContaining({
        args: expect.objectContaining({ runId: "run-re" }),
      }),
    );
    expect(invokeMock).toHaveBeenCalledWith(
      "agent_subscribe_run",
      expect.objectContaining({
        args: expect.objectContaining({
          runId: "run-re",
          afterSequence: 1,
        }),
      }),
    );
    expect(chunks).toEqual([
      { type: "text-delta", id: "c2", delta: " resumed" },
      {
        type: "tool-input-available",
        toolCallId: "t1",
        toolName: "create_proposal",
        input: { title: "Ship A1" },
      },
    ]);
    expect(loadActiveAgentRun("/tmp/workspace", "thread-1")).toBeNull();
  });

  it("reconnectToStream returns null when there is no active run", async () => {
    const transport = new TauriAgentChatTransport({
      workspaceRoot: "/tmp/workspace",
      threadId: "thread-1",
      persistTranscripts: false,
    });
    await expect(transport.reconnectToStream()).resolves.toBeNull();
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("surfaces invoke error even when Channel Error races behind close", async () => {
    invokeMock.mockImplementation(async (_command, payload) => {
      const channel = payload.channel as { onmessage: (message: AgentStreamMsg) => void };
      channel.onmessage({
        kind: "agentEvent",
        runId: "run-err",
        event: { type: "run_started", runId: "run-err", threadId: "thread-1" },
      });
      channel.onmessage({
        kind: "uiChunk",
        runId: "run-err",
        chunk: { type: "tool-input-available", toolCallId: "t1", toolName: "search", input: {} },
      });
      // Simulate the race: invoke returns failure before Channel Error is delivered.
      return {
        runId: "run-err",
        threadId: "thread-1",
        error: "Request contains an invalid argument.",
      };
    });

    const transport = new TauriAgentChatTransport({
      workspaceRoot: "/tmp/workspace",
      threadId: "thread-1",
      persistTranscripts: false,
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
    await expect(
      (async () => {
        while (true) {
          const { done } = await reader.read();
          if (done) {
            throw new Error("stream closed without error");
          }
        }
      })(),
    ).rejects.toThrow(/invalid argument/);
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
      persistTranscripts: false,
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
