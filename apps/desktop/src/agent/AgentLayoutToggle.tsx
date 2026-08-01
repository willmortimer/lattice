import { Button } from "@lattice/ui";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCallback } from "react";

import { hasTauri } from "../lib/ipc";
import { useDesktopUiStore, type AgentLayoutMode } from "../shell/desktopUiStore";
import {
  readAgentDetachedHandoff,
  writeAgentDetachedHandoff,
  type AgentDetachedReturnLayout,
} from "./agentDetachedHandoff";
import {
  AGENT_DETACHED_WINDOW_LABEL,
  requestCloseDetachedAgent,
  showDetachedAgent,
} from "./agentDetachedWindow";
import { useAgentSessionStore } from "./agentStore";

export interface AgentLayoutToggleProps {
  workspaceRoot: string | null;
}

function toReturnLayout(mode: AgentLayoutMode): AgentDetachedReturnLayout {
  switch (mode) {
    case "workbench":
      return "workbench";
    case "focus":
      return "focus";
    case "dock":
    case "detached":
      return "dock";
    default: {
      const _exhaustive: never = mode;
      return _exhaustive;
    }
  }
}

function isDetachedAgentWindow(): boolean {
  if (!hasTauri) {
    return false;
  }
  try {
    return getCurrentWindow().label === AGENT_DETACHED_WINDOW_LABEL;
  } catch {
    return false;
  }
}

export function AgentLayoutToggle({ workspaceRoot }: AgentLayoutToggleProps) {
  const layoutMode = useDesktopUiStore((state) => state.agentLayoutMode);
  const setLayoutMode = useDesktopUiStore((state) => state.setAgentLayoutMode);
  const exitAgentFocus = useDesktopUiStore((state) => state.exitAgentFocus);
  const threadId = useAgentSessionStore((state) =>
    workspaceRoot ? (state.threadIds[workspaceRoot] ?? "") : "",
  );
  const inDetachedWindow = isDetachedAgentWindow();
  const detachedActive = layoutMode === "detached" || inDetachedWindow;

  const selectMode = useCallback(
    (mode: AgentLayoutMode) => {
      if (inDetachedWindow) {
        if (mode === "detached") {
          return;
        }
        const handoff = readAgentDetachedHandoff();
        if (handoff) {
          writeAgentDetachedHandoff({
            ...handoff,
            returnLayoutMode: toReturnLayout(mode),
          });
        }
        void requestCloseDetachedAgent();
        return;
      }

      if (mode === "detached") {
        if (!hasTauri || !workspaceRoot?.trim() || !threadId.trim()) {
          return;
        }
        const returnLayoutMode = toReturnLayout(layoutMode);
        setLayoutMode("detached");
        void showDetachedAgent({
          workspaceRoot,
          threadId,
          returnLayoutMode,
        }).catch(() => {
          setLayoutMode(returnLayoutMode);
        });
        return;
      }

      if (layoutMode === "detached") {
        setLayoutMode(mode);
        const handoff = readAgentDetachedHandoff();
        if (handoff) {
          writeAgentDetachedHandoff({
            ...handoff,
            returnLayoutMode: toReturnLayout(mode),
          });
        }
        void requestCloseDetachedAgent();
        return;
      }

      if (mode === "focus" && layoutMode === "focus") {
        exitAgentFocus();
        return;
      }

      setLayoutMode(mode);
    },
    [exitAgentFocus, inDetachedWindow, layoutMode, setLayoutMode, threadId, workspaceRoot],
  );

  return (
    <div className="agent-layout-toggle" role="group" aria-label="Agent layout mode">
      <Button
        type="button"
        variant="ghost"
        size="sm"
        className={layoutMode === "dock" && !inDetachedWindow ? "agent-layout-toggle-active" : undefined}
        title="Compact sidebar layout"
        aria-pressed={layoutMode === "dock" && !inDetachedWindow}
        onClick={() => selectMode("dock")}
      >
        Dock
      </Button>
      <Button
        type="button"
        variant="ghost"
        size="sm"
        className={
          layoutMode === "workbench" && !inDetachedWindow
            ? "agent-layout-toggle-active"
            : undefined
        }
        title="Split conversation and evidence panes"
        aria-pressed={layoutMode === "workbench" && !inDetachedWindow}
        onClick={() => selectMode("workbench")}
      >
        Workbench
      </Button>
      <Button
        type="button"
        variant="ghost"
        size="sm"
        className={
          layoutMode === "focus" && !inDetachedWindow ? "agent-layout-toggle-active" : undefined
        }
        title="Focus layout (agent fills this window)"
        aria-pressed={layoutMode === "focus" && !inDetachedWindow}
        onClick={() => selectMode("focus")}
      >
        Focus
      </Button>
      <Button
        type="button"
        variant="ghost"
        size="sm"
        className={detachedActive ? "agent-layout-toggle-active" : undefined}
        title="Open agent in a separate window"
        aria-pressed={detachedActive}
        disabled={
          inDetachedWindow || !hasTauri || !workspaceRoot?.trim() || !threadId.trim()
        }
        onClick={() => selectMode("detached")}
      >
        Detached
      </Button>
    </div>
  );
}
