import { Channel } from "@tauri-apps/api/core";
import type { ChatTransport, UIMessage, UIMessageChunk } from "ai";

import { invoke } from "./ipc";

export type AgentHealth = {
  ok: boolean;
  backend: string;
  degraded: boolean;
  model?: string;
};

export type AgentStartRunArgs = {
  workspaceRoot: string;
  threadId: string;
  runId?: string;
  provider?: string;
  model?: string;
  prompt?: string;
  messagesJson?: string;
};

export type AgentStartRunResult = {
  runId: string;
  threadId: string;
  /** Present when the run failed; authoritative over Channel ordering. */
  error?: string;
};

export type AgentStreamMsg =
  | { kind: "uiChunk"; runId: string; chunk: unknown }
  | { kind: "agentEvent"; runId: string; event: unknown }
  | { kind: "done"; runId: string }
  | { kind: "error"; runId: string; message: string };

export async function getAgentHealth(): Promise<AgentHealth> {
  return invoke<AgentHealth>("agent_health");
}

export async function startAgentRun(
  args: AgentStartRunArgs,
  onMessage: (message: AgentStreamMsg) => void,
): Promise<AgentStartRunResult> {
  const channel = new Channel<AgentStreamMsg>((message) => {
    onMessage(message);
  });
  return invoke<AgentStartRunResult>("agent_start_run", { args, channel });
}

export async function cancelAgentRun(runId: string): Promise<void> {
  await invoke<void>("agent_cancel_run", { runId });
}

export type AgentEventHandler = (event: unknown) => void;

export type TauriAgentChatTransportOptions = {
  workspaceRoot: string;
  threadId: string;
  provider?: string;
  model?: string;
  /** Resolve provider/model at send time (UI selectors). */
  resolveRunOptions?: () => { provider?: string; model?: string };
  onAgentEvent?: AgentEventHandler;
};

/** Map one ordered Tauri channel payload into UI chunks and side effects. */
export function applyAgentStreamMessage(
  message: AgentStreamMsg,
  enqueue: (chunk: UIMessageChunk) => void,
  handlers: {
    onAgentEvent?: AgentEventHandler;
    onRunId?: (runId: string) => void;
    onTerminalError?: (message: string) => void;
  },
): "continue" | "done" | "error" {
  switch (message.kind) {
    case "uiChunk":
      enqueue(message.chunk as UIMessageChunk);
      return "continue";
    case "agentEvent": {
      handlers.onAgentEvent?.(message.event);
      if (
        typeof message.event === "object" &&
        message.event !== null &&
        "type" in message.event &&
        message.event.type === "run_started" &&
        "runId" in message.event &&
        typeof message.event.runId === "string"
      ) {
        handlers.onRunId?.(message.event.runId);
      }
      return "continue";
    }
    case "done":
      return "done";
    case "error":
      handlers.onTerminalError?.(message.message);
      return "error";
    default: {
      const _exhaustive: never = message;
      return _exhaustive;
    }
  }
}

/**
 * AI SDK transport that streams agent runs through Tauri commands and ordered
 * Channels. Provider secrets never enter the webview.
 */
export class TauriAgentChatTransport implements ChatTransport<UIMessage> {
  private activeRunId: string | null = null;

  constructor(private readonly options: TauriAgentChatTransportOptions) {}

  getActiveRunId(): string | null {
    return this.activeRunId;
  }

  async sendMessages({
    chatId,
    messages,
    abortSignal,
  }: Parameters<ChatTransport<UIMessage>["sendMessages"]>[0]): Promise<
    ReadableStream<UIMessageChunk>
  > {
    const threadId = this.options.threadId || chatId;
    const messagesJson = JSON.stringify(messages);

    return new ReadableStream<UIMessageChunk>({
      start: (controller) => {
        let closed = false;
        const closeOnce = () => {
          if (!closed) {
            closed = true;
            controller.close();
          }
        };
        const failOnce = (error: unknown) => {
          if (!closed) {
            closed = true;
            controller.error(error);
          }
        };

        const onAbort = () => {
          const runId = this.activeRunId;
          if (runId) {
            void cancelAgentRun(runId);
          }
        };
        abortSignal?.addEventListener("abort", onAbort);

        void (async () => {
          try {
            const resolved = this.options.resolveRunOptions?.() ?? {};
            const result = await startAgentRun(
              {
                workspaceRoot: this.options.workspaceRoot,
                threadId,
                provider: resolved.provider ?? this.options.provider,
                model: resolved.model ?? this.options.model,
                messagesJson,
              },
              (message) => {
                const outcome = applyAgentStreamMessage(message, (chunk) => {
                  controller.enqueue(chunk);
                }, {
                  onAgentEvent: this.options.onAgentEvent,
                  onRunId: (runId) => {
                    this.activeRunId = runId;
                  },
                  onTerminalError: (message) => {
                    failOnce(new Error(message));
                  },
                });

                // Channel Done/Error can race the invoke promise. Prefer the
                // returned `error` field below when the stream is still open.
                if (outcome === "done") {
                  closeOnce();
                }
              },
            );
            if (result.error) {
              failOnce(new Error(result.error));
            } else {
              closeOnce();
            }
          } catch (error) {
            failOnce(error);
          } finally {
            abortSignal?.removeEventListener("abort", onAbort);
            this.activeRunId = null;
          }
        })();
      },
    });
  }

  async reconnectToStream(): Promise<ReadableStream<UIMessageChunk> | null> {
    return null;
  }

  /** Cancel the active run when `useChat` stop() aborts the transport. */
  async stop(): Promise<void> {
    const runId = this.activeRunId;
    if (!runId) {
      return;
    }
    await cancelAgentRun(runId);
  }
}
