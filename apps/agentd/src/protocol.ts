/**
 * Thin wrappers around `@lattice/agent-protocol` for the JSONL sidecar wire format.
 */
import {
  PROTOCOL_VERSION,
  parseCommand,
  serializeCommand,
  parseEvent,
  serializeEvent,
  type AgentCommand,
  type AgentEvent,
  type StartRunCommand,
  type CancelRunCommand,
  type ProviderKind,
  type UiMessageChunk,
} from "@lattice/agent-protocol";

export {
  PROTOCOL_VERSION,
  parseCommand,
  serializeCommand,
  parseEvent,
  serializeEvent,
  type AgentCommand,
  type AgentEvent,
  type StartRunCommand,
  type CancelRunCommand,
  type ProviderKind,
  type UiMessageChunk,
};

/** Write one JSONL event to stdout. */
export function emitEvent(
  event: AgentEvent,
  write: (line: string) => void = (line) => {
    process.stdout.write(`${line}\n`);
  },
): void {
  write(serializeEvent(event));
}

/** Write a diagnostic line to stderr (never stdout). */
export function logDiag(message: string, err?: unknown): void {
  if (err === undefined) {
    process.stderr.write(`[agentd] ${message}\n`);
    return;
  }
  const detail = err instanceof Error ? err.stack ?? err.message : String(err);
  process.stderr.write(`[agentd] ${message}: ${detail}\n`);
}
