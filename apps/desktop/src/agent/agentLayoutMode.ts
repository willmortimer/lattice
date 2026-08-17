import type { AgentLayoutMode } from "../shell/desktopUiStore";

import type { AgentDetachedReturnLayout } from "./agentDetachedHandoff";

export const AGENT_LAYOUT_MODES = ["dock", "workbench", "focus", "detached"] as const;

export const AGENT_LAYOUT_MODE_LABEL: Record<AgentLayoutMode, string> = {
  dock: "Dock",
  workbench: "Workbench",
  focus: "Focus",
  detached: "Detached",
};

export const AGENT_LAYOUT_MODE_TITLE: Record<AgentLayoutMode, string> = {
  dock: "Compact sidebar beside your files",
  workbench: "Split conversation and evidence panes",
  focus: "Agent fills this window",
  detached: "Open agent in a separate window",
};

export function agentLayoutModeLabel(mode: AgentLayoutMode): string {
  return AGENT_LAYOUT_MODE_LABEL[mode];
}

/** Layout to restore when a detached agent window closes. */
export function toDetachedReturnLayout(mode: AgentLayoutMode): AgentDetachedReturnLayout {
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
