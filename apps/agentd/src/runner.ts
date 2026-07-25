import { run } from "@openai/agents";
import { createAiSdkUiMessageStream } from "@openai/agents-extensions/ai-sdk-ui";

import { createWorkspaceAgent } from "./agent.js";
import type { AgentdConfig } from "./config.js";
import { resolveProvider } from "./config.js";
import { streamFakeChunks } from "./fake-provider.js";
import {
  emitEvent,
  logDiag,
  type AgentEvent,
  type StartRunCommand,
  type UiMessageChunk,
} from "./protocol.js";
import { configureProvider } from "./provider.js";
import type { LatticeRunContext } from "./tools.js";

export type EventSink = (event: AgentEvent) => void;

function isAbortError(err: unknown): boolean {
  if (typeof err === "object" && err !== null && "name" in err) {
    return (err as { name?: string }).name === "AbortError";
  }
  return false;
}

export function promptFromCommand(command: StartRunCommand): string {
  if (command.prompt !== undefined && command.prompt.length > 0) {
    return command.prompt;
  }
  const messages = command.messages ?? [];
  const parts: string[] = [];
  for (const message of messages) {
    const role = message.role;
    const rawContent = (message as { content?: unknown }).content;
    const content =
      typeof rawContent === "string" ? rawContent : JSON.stringify(message);
    parts.push(`${role}: ${content}`);
  }
  if (parts.length === 0) {
    throw new Error("start_run requires a non-empty prompt or messages");
  }
  return parts.join("\n");
}

async function emitUiStream(
  runId: string,
  stream: ReadableStream<UiMessageChunk>,
  sink: EventSink,
  signal: AbortSignal,
): Promise<void> {
  const reader = stream.getReader();
  const onAbort = () => {
    void reader.cancel("cancelled");
  };
  signal.addEventListener("abort", onAbort, { once: true });
  try {
    while (!signal.aborted) {
      const { done, value } = await reader.read();
      if (done) {
        break;
      }
      // Tools execute inside agentd; mark providerExecuted so AI SDK / useChat
      // does not await a client-side onToolCall before consuming later chunks.
      sink({
        type: "message_chunk",
        runId,
        chunk: markProviderExecutedToolChunk(value),
      });
    }
  } finally {
    signal.removeEventListener("abort", onAbort);
    reader.releaseLock();
  }
}

function markProviderExecutedToolChunk(chunk: UiMessageChunk): UiMessageChunk {
  if (typeof chunk !== "object" || chunk === null || !("type" in chunk)) {
    return chunk;
  }
  const type = (chunk as { type?: unknown }).type;
  if (
    type !== "tool-input-start" &&
    type !== "tool-input-delta" &&
    type !== "tool-input-available" &&
    type !== "tool-input-error" &&
    type !== "tool-output-available" &&
    type !== "tool-output-error"
  ) {
    return chunk;
  }
  return { ...(chunk as Record<string, unknown>), providerExecuted: true } as UiMessageChunk;
}

async function streamRun(
  config: AgentdConfig,
  command: StartRunCommand,
  sink: EventSink,
  abort: AbortController,
  chunkDelayMs: number,
): Promise<void> {
  const { runId, threadId } = command;
  const provider = resolveProvider(config, command.provider);
  const model = command.model || config.defaultModel;

  sink({ type: "run_started", runId, threadId, provider });

  try {
    const prompt = promptFromCommand(command);

    if (provider === "fake") {
      for await (const chunk of streamFakeChunks(prompt, {
        chunkDelayMs,
        signal: abort.signal,
      })) {
        sink({ type: "message_chunk", runId, chunk });
      }
      if (abort.signal.aborted) {
        sink({
          type: "run_failed",
          runId,
          message: "Run cancelled",
          retryable: false,
        });
        return;
      }
      sink({ type: "run_completed", runId });
      return;
    }

    configureProvider(config, provider, model);
    const agent = createWorkspaceAgent(model);
    const latticeContext: LatticeRunContext = {
      client: config.latticeClient,
      workspaceId: command.workspaceId,
      workspaceRoot: command.workspaceRoot,
      runId,
      emitEvent: sink,
    };
    if (!config.latticeClient) {
      logDiag(
        "Lattice HTTP tools unavailable: set LATTICE_API_BASE_URL and LATTICE_AUTH_TOKEN",
      );
    }
    const streamed = await run(agent, prompt, {
      stream: true,
      signal: abort.signal,
      context: latticeContext,
    });
    const uiStream = createAiSdkUiMessageStream(streamed) as ReadableStream<UiMessageChunk>;
    await emitUiStream(runId, uiStream, sink, abort.signal);

    if (abort.signal.aborted) {
      sink({
        type: "run_failed",
        runId,
        message: "Run cancelled",
        retryable: false,
      });
      return;
    }

    sink({ type: "run_completed", runId });
  } catch (err) {
    if (isAbortError(err) || abort.signal.aborted) {
      sink({
        type: "run_failed",
        runId,
        message: "Run cancelled",
        retryable: false,
      });
      return;
    }
    const message = err instanceof Error ? err.message : String(err);
    logDiag(`run ${runId} failed`, err);
    sink({
      type: "run_failed",
      runId,
      message,
      retryable: true,
    });
  }
}

type ActiveRun = {
  runId: string;
  abort: AbortController;
  done: Promise<void>;
};

/**
 * Tracks at most one active run and supports cancel_run via AbortController.
 */
export class RunManager {
  private active: ActiveRun | null = null;
  private readonly config: AgentdConfig;
  private readonly sink: EventSink;
  private readonly chunkDelayMs: number;

  constructor(
    config: AgentdConfig,
    sink: EventSink = emitEvent,
    options: { chunkDelayMs?: number } = {},
  ) {
    this.config = config;
    this.sink = sink;
    this.chunkDelayMs = options.chunkDelayMs ?? 0;
  }

  get activeRunId(): string | null {
    return this.active?.runId ?? null;
  }

  async start(command: StartRunCommand): Promise<void> {
    if (this.active !== null) {
      // Preempt a stuck prior run so the UI cannot wedge on "another run is active".
      logDiag(
        `preempting active run ${this.active.runId} for ${command.runId}`,
      );
      this.active.abort.abort();
      try {
        await this.active.done;
      } catch {
        // Prior run failure is expected after abort.
      }
      this.active = null;
    }

    const abort = new AbortController();
    // Register before awaiting so cancel_run can abort mid-stream.
    const runState: ActiveRun = {
      runId: command.runId,
      abort,
      done: Promise.resolve(),
    };
    this.active = runState;
    logDiag(
      `start_run ${command.runId} provider=${command.provider} model=${command.model || this.config.defaultModel} workspaceId=${command.workspaceId ?? ""}`,
    );
    const done = streamRun(
      this.config,
      command,
      this.sink,
      abort,
      this.chunkDelayMs,
    );
    runState.done = done;

    try {
      await done;
    } finally {
      if (this.active?.runId === command.runId) {
        this.active = null;
      }
    }
  }

  cancel(runId: string): void {
    if (this.active === null || this.active.runId !== runId) {
      logDiag(`cancel_run ignored; no active run ${runId}`);
      return;
    }
    this.active.abort.abort();
  }

  async waitForIdle(): Promise<void> {
    if (this.active !== null) {
      await this.active.done;
    }
  }
}
