import type { ReactNode } from "react";

export interface AgentPanelShellProps {
  children: ReactNode;
}

export function AgentPanelShell({ children }: AgentPanelShellProps) {
  return (
    <aside className="agent-panel" aria-label="Agent">
      {children}
    </aside>
  );
}
