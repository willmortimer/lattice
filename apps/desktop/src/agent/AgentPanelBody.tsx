import type { ReactNode } from "react";

import { useDesktopUiStore } from "../shell/desktopUiStore";
import { AgentTrail } from "./AgentTrail";
import { AgentWorkbenchLayout } from "./AgentWorkbenchLayout";

export interface AgentPanelBodyProps {
  thread: ReactNode;
}

export function AgentPanelBody({ thread }: AgentPanelBodyProps) {
  const layoutMode = useDesktopUiStore((state) => state.agentLayoutMode);

  switch (layoutMode) {
    case "dock":
      return (
        <>
          <AgentTrail />
          {thread}
        </>
      );
    case "workbench":
      return <AgentWorkbenchLayout conversation={thread} />;
    default: {
      const _exhaustive: never = layoutMode;
      return _exhaustive;
    }
  }
}
