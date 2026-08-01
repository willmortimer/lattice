import { Channel } from "@tauri-apps/api/core";
import type { ChatTransport, UIMessage, UIMessageChunk } from "ai";

import {
  persistAgentRunTranscript,
  RunTranscriptAccumulator,
} from "../agent/agentTranscriptPersistence";
import {
  clearActiveAgentRun,
  loadActiveAgentRun,
  persistActiveAgentRun,
  updateActiveAgentRunSequence,
} from "./agentActiveRun";
import {
  getAgentRunStatus,
  isTerminalAgentRunStatus,
  subscribeAgentRun,
} from "./agentRunEvents";
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
  /** When false, skip durable thread persistence (tests). Defaults to true. */
  persistTranscripts?: boolean;
  /** When false, skip active-run sessionStorage (tests). Defaults to true. */
  persistActiveRun?: boolean;
};

function scheduleAgentRunTranscriptPersistence(args: {
  workspaceRoot: string;
  threadId: string;
  messages: UIMessage[];
  accumulator: RunTranscriptAccumulator;
  runId: string;
  error?: string;
}): void {
  const snapshot = args.accumulator.snapshot();
  const runId = snapshot.runId ?? args.runId;
  const error = args.error ?? snapshot.streamError ?? undefined;
  void persistAgentRunTranscript({
    workspaceRoot: args.workspaceRoot,
    threadId: args.threadId,
    messages: args.messages,
    chunks: snapshot.chunks,
    runId,
    error,
  }).catch(() => {
    // Persistence must not block or surface in the composer.
  });
}

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
  private afterSequence = 0;

  constructor(private readonly options: TauriAgentChatTransportOptions) {}

  getActiveRunId(): string | null {
    return this.activeRunId;
  }

  private shouldPersistActiveRun(): boolean {
    return this.options.persistActiveRun !== false;
  }

  private rememberActiveRun(runId: string, afterSequence = this.afterSequence): void {
    this.activeRunId = runId;
    this.afterSequence = afterSequence;
    if (!this.shouldPersistActiveRun()) {
      return;
    }
    persistActiveAgentRun({
      workspaceRoot: this.options.workspaceRoot,
      threadId: this.options.threadId,
      runId,
      afterSequence,
    });
  }

  private clearRememberedActiveRun(): void {
    this.activeRunId = null;
    this.afterSequence = 0;
    if (!this.shouldPersistActiveRun()) {
      return;
    }
    clearActiveAgentRun(this.options.workspaceRoot, this.options.threadId);
  }

  private advanceAckCursor(afterSequence: number): void {
    if (afterSequence <= this.afterSequence) {
      return;
    }
    this.afterSequence = afterSequence;
    if (!this.activeRunId || !this.shouldPersistActiveRun()) {
      return;
    }
    updateActiveAgentRunSequence(
      this.options.workspaceRoot,
      this.options.threadId,
      afterSequence,
    );
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
          const persistTranscripts = this.options.persistTranscripts !== false;
          const transcript = new RunTranscriptAccumulator();
          // Explicit `string`: crypto.randomUUID() is a branded template type in TS DOM libs.
          let completedRunId: string = crypto.randomUUID();
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
                if (persistTranscripts) {
                  transcript.observe(message);
                }
                const outcome = applyAgentStreamMessage(message, (chunk) => {
                  controller.enqueue(chunk);
                }, {
                  onAgentEvent: this.options.onAgentEvent,
                  onRunId: (runId) => {
                    completedRunId = runId;
                    this.rememberActiveRun(runId, 0);
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
            completedRunId = result.runId;
            this.rememberActiveRun(result.runId, this.afterSequence);
            if (persistTranscripts) {
              scheduleAgentRunTranscriptPersistence({
                workspaceRoot: this.options.workspaceRoot,
                threadId,
                messages,
                accumulator: transcript,
                runId: result.runId,
                error: result.error,
              });
            }
            if (result.error) {
              this.clearRememberedActiveRun();
              failOnce(new Error(result.error));
            } else {
              this.clearRememberedActiveRun();
              closeOnce();
            }
          } catch (error) {
            if (persistTranscripts) {
              scheduleAgentRunTranscriptPersistence({
                workspaceRoot: this.options.workspaceRoot,
                threadId,
                messages,
                accumulator: transcript,
                runId: completedRunId,
                error: error instanceof Error ? error.message : String(error),
              });
            }
            this.clearRememberedActiveRun();
            failOnce(error);
          } finally {
            abortSignal?.removeEventListener("abort", onAbort);
          }
        })();
      },
    });
  }

  /**
   * Resume an in-flight run: status → subscribe(after_sequence) → replay → live-tail.
   * Returns null when there is no active run (or nothing left to stream).
   */
  async reconnectToStream(): Promise<ReadableStream<UIMessageChunk> | null> {
    const persisted = this.shouldPersistActiveRun()
      ? loadActiveAgentRun(this.options.workspaceRoot, this.options.threadId)
      : null;
    const runId = this.activeRunId ?? persisted?.runId ?? null;
    if (!runId) {
      return null;
    }
    const afterSequence = Math.max(
      this.afterSequence,
      persisted?.afterSequence ?? 0,
    );

    let status;
    try {
      status = await getAgentRunStatus({
        workspaceRoot: this.options.workspaceRoot,
        runId,
      });
    } catch {
      return null;
    }
    if (!status.run) {
      this.clearRememberedActiveRun();
      return null;
    }

    // Nothing left to deliver when terminal and cursor is caught up.
    if (
      isTerminalAgentRunStatus(status.run.status) &&
      afterSequence >= status.run.lastSequence
    ) {
      this.clearRememberedActiveRun();
      return null;
    }

    this.rememberActiveRun(runId, afterSequence);

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

        void (async () => {
          try {
            const result = await subscribeAgentRun(
              {
                workspaceRoot: this.options.workspaceRoot,
                runId,
                afterSequence,
              },
              (message) => {
                const outcome = applyAgentStreamMessage(message, (chunk) => {
                  controller.enqueue(chunk);
                }, {
                  onAgentEvent: this.options.onAgentEvent,
                  onRunId: (id) => {
                    this.rememberActiveRun(id, this.afterSequence);
                  },
                  onTerminalError: (message) => {
                    failOnce(new Error(message));
                  },
                });
                if (outcome === "done") {
                  closeOnce();
                }
              },
            );
            this.advanceAckCursor(result.lastSequence);
            if (result.error) {
              this.clearRememberedActiveRun();
              failOnce(new Error(result.error));
            } else {
              this.clearRememberedActiveRun();
              closeOnce();
            }
          } catch (error) {
            failOnce(error);
          }
        })();
      },
    });
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
