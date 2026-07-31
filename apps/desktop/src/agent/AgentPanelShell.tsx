import { useEffect, type ReactNode } from "react";

import { emitProductTelemetry } from "../lib/cloud";

export interface AgentPanelShellProps {
  children: ReactNode;
}

export function AgentPanelShell({ children }: AgentPanelShellProps) {
  useEffect(() => {
    void emitProductTelemetry("agent_panel_opened");
  }, []);

  return (
    <aside className="agent-panel" aria-label="Agent">
      {children}
    </aside>
  );
}
