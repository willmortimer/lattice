import { IconButton } from "@lattice/ui";
import { X } from "@phosphor-icons/react";
import { useEffect, useRef, type ReactNode } from "react";

import { inBrowser } from "../demo";
import { TerminalHost } from "./TerminalHost";
import "./terminal.css";
import "@xterm/xterm/css/xterm.css";

export interface TerminalPanelProps {
  workspaceRoot: string | null;
  hasTerminalCapability: boolean;
  onClose: () => void;
}

export function TerminalPanel({
  workspaceRoot,
  hasTerminalCapability,
  onClose,
}: TerminalPanelProps) {
  const hostRef = useRef<HTMLDivElement | null>(null);
  const terminalRef = useRef<TerminalHost | null>(null);

  const canSpawn =
    !inBrowser && hasTerminalCapability && Boolean(workspaceRoot?.trim());

  useEffect(() => {
    if (!canSpawn || !hostRef.current) return;

    const host = new TerminalHost({
      root: workspaceRoot!,
      container: hostRef.current,
    });
    terminalRef.current = host;

    void host.start().catch(() => {
      // Spawn failures surface when Tauri IPC is unavailable or denied.
    });

    return () => {
      host.dispose();
      terminalRef.current = null;
    };
  }, [canSpawn, workspaceRoot]);

  let body: ReactNode;
  if (inBrowser) {
    body = (
      <div className="terminal-dock-placeholder" role="status">
        <p>Terminal is available in the native Lattice desktop app.</p>
      </div>
    );
  } else if (!hasTerminalCapability) {
    body = (
      <div className="terminal-dock-placeholder" role="status">
        <p>Enable the Terminal capability in Settings to use the embedded shell.</p>
      </div>
    );
  } else if (!workspaceRoot) {
    body = (
      <div className="terminal-dock-placeholder" role="status">
        <p>Open a workspace to start a terminal session.</p>
      </div>
    );
  } else {
    body = <div ref={hostRef} className="terminal-dock-host" />;
  }

  return (
    <section className="terminal-dock" aria-label="Terminal">
      <header className="terminal-dock-head">
        <span className="terminal-dock-title">Terminal</span>
        <IconButton label="Close terminal" onClick={onClose}>
          <X size={14} />
        </IconButton>
      </header>
      <div className="terminal-dock-body">{body}</div>
    </section>
  );
}
