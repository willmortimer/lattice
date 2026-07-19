import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import { inBrowser } from "../demo";
import { hasTauri, invoke } from "./ipc";

export { decodeTerminalOutput } from "./terminalPayload";

export interface TerminalSpawnInput {
  root: string;
  cols: number;
  rows: number;
}

export interface TerminalSpawnResult {
  sessionId: string;
}

export interface TerminalWriteInput {
  sessionId: string;
  data: string;
}

export interface TerminalResizeInput {
  sessionId: string;
  cols: number;
  rows: number;
}

export interface TerminalKillInput {
  sessionId: string;
}

export interface TerminalOutputEvent {
  sessionId: string;
  data: number[];
}

export interface TerminalExitEvent {
  sessionId: string;
  code: number | null;
}

export class TerminalUnavailableError extends Error {
  constructor() {
    super("Terminal is not available outside the native desktop shell.");
    this.name = "TerminalUnavailableError";
  }
}

function assertTerminalAvailable(): void {
  if (inBrowser || !hasTauri) {
    throw new TerminalUnavailableError();
  }
}

export async function terminalSpawn(input: TerminalSpawnInput): Promise<TerminalSpawnResult> {
  assertTerminalAvailable();
  return invoke<TerminalSpawnResult>("terminal_spawn", {
    root: input.root,
    cols: input.cols,
    rows: input.rows,
  });
}

export async function terminalWrite(input: TerminalWriteInput): Promise<void> {
  assertTerminalAvailable();
  await invoke("terminal_write", {
    sessionId: input.sessionId,
    data: input.data,
  });
}

export async function terminalResize(input: TerminalResizeInput): Promise<void> {
  assertTerminalAvailable();
  await invoke("terminal_resize", {
    sessionId: input.sessionId,
    cols: input.cols,
    rows: input.rows,
  });
}

export async function terminalKill(input: TerminalKillInput): Promise<void> {
  assertTerminalAvailable();
  await invoke("terminal_kill", {
    sessionId: input.sessionId,
  });
}

export async function listenTerminalOutput(
  handler: (event: TerminalOutputEvent) => void,
): Promise<UnlistenFn> {
  assertTerminalAvailable();
  return listen<TerminalOutputEvent>("terminal-output", (event) => {
    handler(event.payload);
  });
}

export async function listenTerminalExit(
  handler: (event: TerminalExitEvent) => void,
): Promise<UnlistenFn> {
  assertTerminalAvailable();
  return listen<TerminalExitEvent>("terminal-exit", (event) => {
    handler(event.payload);
  });
}
