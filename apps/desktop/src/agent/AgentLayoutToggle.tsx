import { Button } from "@lattice/ui";

import { useDesktopUiStore } from "../shell/desktopUiStore";

export function AgentLayoutToggle() {
  const layoutMode = useDesktopUiStore((state) => state.agentLayoutMode);
  const setLayoutMode = useDesktopUiStore((state) => state.setAgentLayoutMode);

  return (
    <div className="agent-layout-toggle" role="group" aria-label="Agent layout mode">
      <Button
        type="button"
        variant="ghost"
        size="sm"
        className={layoutMode === "dock" ? "agent-layout-toggle-active" : undefined}
        title="Compact sidebar layout"
        aria-pressed={layoutMode === "dock"}
        onClick={() => setLayoutMode("dock")}
      >
        Dock
      </Button>
      <Button
        type="button"
        variant="ghost"
        size="sm"
        className={layoutMode === "workbench" ? "agent-layout-toggle-active" : undefined}
        title="Split conversation and evidence panes"
        aria-pressed={layoutMode === "workbench"}
        onClick={() => setLayoutMode("workbench")}
      >
        Workbench
      </Button>
      <Button
        type="button"
        variant="ghost"
        size="sm"
        disabled
        title="Detached agent window (coming soon)"
      >
        Detached
      </Button>
    </div>
  );
}
