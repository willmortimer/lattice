import {
  Button,
  MenuItem,
  MenuPopup,
  MenuPortal,
  MenuPositioner,
  MenuRoot,
  MenuTrigger,
} from "@lattice/ui";
import { CaretDown, Check } from "@phosphor-icons/react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCallback } from "react";

import { hasTauri } from "../lib/ipc";
import { useDesktopUiStore, type AgentLayoutMode } from "../shell/desktopUiStore";
import { readAgentDetachedHandoff, writeAgentDetachedHandoff } from "./agentDetachedHandoff";
import {
  AGENT_DETACHED_WINDOW_LABEL,
  requestCloseDetachedAgent,
  showDetachedAgent,
} from "./agentDetachedWindow";
import {
  AGENT_LAYOUT_MODES,
  AGENT_LAYOUT_MODE_TITLE,
  agentLayoutModeLabel,
  toDetachedReturnLayout,
} from "./agentLayoutMode";
import { useAgentSessionStore } from "./agentStore";

export interface AgentLayoutToggleProps {
  workspaceRoot: string | null;
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
  const threadId = useAgentSessionStore((state) =>
    workspaceRoot ? (state.threadIds[workspaceRoot] ?? "") : "",
  );
  const inDetachedWindow = isDetachedAgentWindow();
  const visibleMode: AgentLayoutMode = inDetachedWindow ? "detached" : layoutMode;
  const detachedDisabled =
    inDetachedWindow || !hasTauri || !workspaceRoot?.trim() || !threadId.trim();

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
            returnLayoutMode: toDetachedReturnLayout(mode),
          });
        }
        void requestCloseDetachedAgent();
        return;
      }

      if (mode === "detached") {
        if (!hasTauri || !workspaceRoot?.trim() || !threadId.trim()) {
          return;
        }
        const returnLayoutMode = toDetachedReturnLayout(layoutMode);
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
            returnLayoutMode: toDetachedReturnLayout(mode),
          });
        }
        void requestCloseDetachedAgent();
        return;
      }

      if (mode === layoutMode) {
        return;
      }

      setLayoutMode(mode);
    },
    [inDetachedWindow, layoutMode, setLayoutMode, threadId, workspaceRoot],
  );

  return (
    <MenuRoot>
      <MenuTrigger
        render={
          <Button
            type="button"
            variant="ghost"
            size="sm"
            className="agent-layout-toggle-trigger"
            title="Agent layout"
            aria-label={`Agent layout, ${agentLayoutModeLabel(visibleMode)}`}
          />
        }
      >
        {agentLayoutModeLabel(visibleMode)}
        <CaretDown size={10} weight="bold" />
      </MenuTrigger>
      <MenuPortal>
        <MenuPositioner sideOffset={4} align="end">
          <MenuPopup className="ltui-menu agent-layout-toggle-menu">
            {AGENT_LAYOUT_MODES.map((mode) => {
              const selected = mode === visibleMode;
              return (
                <MenuItem
                  key={mode}
                  className="ltui-menu-item"
                  disabled={mode === "detached" ? detachedDisabled && !selected : false}
                  title={AGENT_LAYOUT_MODE_TITLE[mode]}
                  aria-checked={selected}
                  onClick={() => {
                    selectMode(mode);
                  }}
                >
                  <span className="agent-layout-toggle-check" aria-hidden="true">
                    {selected ? <Check size={12} weight="bold" /> : null}
                  </span>
                  {agentLayoutModeLabel(mode)}
                </MenuItem>
              );
            })}
          </MenuPopup>
        </MenuPositioner>
      </MenuPortal>
    </MenuRoot>
  );
}
