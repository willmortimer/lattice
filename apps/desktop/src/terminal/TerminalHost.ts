import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "@xterm/xterm";
import type { UnlistenFn } from "@tauri-apps/api/event";

import {
  listenTerminalExit,
  listenTerminalOutput,
  terminalKill,
  terminalResize,
  terminalSpawn,
  terminalWrite,
} from "../lib/terminal";
import { decodeTerminalOutput } from "../lib/terminalPayload";
import { latticeTerminalTheme } from "./terminalTheme";

export interface TerminalHostOptions {
  root: string;
  container: HTMLElement;
}

/**
 * Imperative xterm.js host. React mounts the container; bytes never flow through
 * React state (ADR 0039).
 */
export class TerminalHost {
  private readonly term: Terminal;
  private readonly fitAddon: FitAddon;
  private sessionId: string | null = null;
  private disposed = false;
  private unlistenOutput: UnlistenFn | null = null;
  private unlistenExit: UnlistenFn | null = null;
  private resizeObserver: ResizeObserver | null = null;
  private dataDisposable: { dispose(): void } | null = null;

  constructor(private readonly options: TerminalHostOptions) {
    this.term = new Terminal({
      cursorBlink: true,
      fontFamily: "var(--lt-font-mono)",
      fontSize: 12,
      lineHeight: 1.35,
      theme: latticeTerminalTheme(),
      scrollback: 5000,
    });
    this.fitAddon = new FitAddon();
    this.term.loadAddon(this.fitAddon);
    this.term.open(options.container);
  }

  async start(): Promise<void> {
    if (this.disposed) return;

    this.fitAddon.fit();
    const { sessionId } = await terminalSpawn({
      root: this.options.root,
      cols: this.term.cols,
      rows: this.term.rows,
    });
    this.sessionId = sessionId;

    this.unlistenOutput = await listenTerminalOutput((event) => {
      if (event.sessionId !== this.sessionId || this.disposed) return;
      this.term.write(decodeTerminalOutput(event.data));
    });

    this.unlistenExit = await listenTerminalExit((event) => {
      if (event.sessionId !== this.sessionId || this.disposed) return;
      const code = event.code;
      const suffix = code === null ? "" : ` (exit ${code})`;
      this.term.writeln(`\r\n[process exited${suffix}]`);
    });

    this.dataDisposable = this.term.onData((data) => {
      if (!this.sessionId || this.disposed) return;
      void terminalWrite({ sessionId: this.sessionId, data }).catch(() => {
        // Spawn/IPC errors surface through invoke; write failures are non-fatal.
      });
    });

    this.resizeObserver = new ResizeObserver(() => {
      if (this.disposed) return;
      this.fitAddon.fit();
      if (!this.sessionId) return;
      void terminalResize({
        sessionId: this.sessionId,
        cols: this.term.cols,
        rows: this.term.rows,
      });
    });
    this.resizeObserver.observe(this.options.container);
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;

    this.resizeObserver?.disconnect();
    this.resizeObserver = null;

    this.dataDisposable?.dispose();
    this.dataDisposable = null;

    this.unlistenOutput?.();
    this.unlistenOutput = null;

    this.unlistenExit?.();
    this.unlistenExit = null;

    const sessionId = this.sessionId;
    this.sessionId = null;
    if (sessionId) {
      void terminalKill({ sessionId });
    }

    this.term.dispose();
  }
}
