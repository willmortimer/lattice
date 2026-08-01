import { useEffect, type ReactNode } from "react";

import { emitProductTelemetry } from "../lib/cloud";
import { useDesktopUiStore } from "../shell/desktopUiStore";

export interface AgentPanelShellProps {
  children: ReactNode;
}

export function AgentPanelShell({ children }: AgentPanelShellProps) {
  const layoutMode = useDesktopUiStore((state) => state.agentLayoutMode);

  useEffect(() => {
    void emitProductTelemetry("agent_panel_opened");
  }, []);

  return (
    <aside
      className={`agent-panel${
        layoutMode === "workbench" || layoutMode === "focus"
          ? " agent-panel-workbench"
          : layoutMode === "detached"
            ? " agent-panel-detached"
            : ""
      }`}
      data-agent-layout={layoutMode}
      aria-label="Agent"
    >
      {children}
    </aside>
  );
}
