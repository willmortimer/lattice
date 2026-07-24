import readline from "node:readline";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { loadConfig } from "./config.js";
import {
  PROTOCOL_VERSION,
  emitEvent,
  logDiag,
  parseCommand,
  type AgentCommand,
  type AgentEvent,
} from "./protocol.js";
import { RunManager } from "./runner.js";

export type AgentdLoopOptions = {
  input?: NodeJS.ReadableStream;
  /** Called with each outbound JSONL event line (without trailing newline). */
  onEventLine?: (line: string) => void;
  /** Override config (tests). */
  config?: ReturnType<typeof loadConfig>;
  /** Delay between fake chunks (tests). */
  chunkDelayMs?: number;
};

/**
 * JSONL command loop: stdin commands → stdout events.
 * Returns when `shutdown` is received or the input stream ends.
 */
export async function runAgentdLoop(
  options: AgentdLoopOptions = {},
): Promise<void> {
  const config = options.config ?? loadConfig();
  const writeEvent =
    options.onEventLine !== undefined
      ? (line: string) => {
          options.onEventLine!(line);
        }
      : (line: string) => {
          process.stdout.write(`${line}\n`);
        };

  const sink = (event: AgentEvent) => {
    emitEvent(event, writeEvent);
  };

  const runs = new RunManager(config, sink, {
    chunkDelayMs: options.chunkDelayMs ?? 0,
  });

  // Announce readiness so supervisors can wait for a first hello_ack after hello.
  logDiag(
    `ready provider=${config.defaultProvider} model=${config.defaultModel} fake=${config.forceFake}`,
  );

  const input = options.input ?? process.stdin;
  const rl = readline.createInterface({ input, crlfDelay: Infinity });

  let shuttingDown = false;

  const handle = async (command: AgentCommand): Promise<boolean> => {
    switch (command.type) {
      case "hello":
        sink({
          type: "hello_ack",
          protocolVersion: PROTOCOL_VERSION,
        });
        return false;
      case "health":
        sink({ type: "health", ok: true });
        return false;
      case "shutdown":
        shuttingDown = true;
        if (runs.activeRunId !== null) {
          runs.cancel(runs.activeRunId);
        }
        await runs.waitForIdle();
        return true;
      case "cancel_run":
        runs.cancel(command.runId);
        return false;
      case "start_run":
        // Do not await completion so cancel_run can arrive concurrently.
        void runs.start(command).catch((err) => {
          logDiag("unhandled start_run error", err);
        });
        return false;
      default: {
        const _exhaustive: never = command;
        throw new Error(`Unhandled command: ${JSON.stringify(_exhaustive)}`);
      }
    }
  };

  for await (const line of rl) {
    if (shuttingDown) {
      break;
    }
    const trimmed = line.trim();
    if (trimmed.length === 0) {
      continue;
    }
    try {
      const command = parseCommand(trimmed);
      const shouldExit = await handle(command);
      if (shouldExit) {
        break;
      }
    } catch (err) {
      logDiag("failed to handle command line", err);
      // Protocol parse failures are diagnostics only; keep the loop alive.
    }
  }

  rl.close();
  await runs.waitForIdle();
}

async function main(): Promise<void> {
  try {
    await runAgentdLoop();
  } catch (err) {
    logDiag("fatal", err);
    process.exitCode = 1;
  }
}

const entryPath = path.resolve(fileURLToPath(import.meta.url));
const isCliEntry = process.argv.some((arg) => {
  try {
    return path.resolve(arg) === entryPath;
  } catch {
    return false;
  }
});
if (isCliEntry) {
  void main();
}
