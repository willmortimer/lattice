import { Group, Panel, Separator } from "react-resizable-panels";
import type { ReactNode } from "react";

import { useDesktopUiStore } from "../shell/desktopUiStore";
import { AgentWorkbenchPane, type AgentWorkbenchPaneProps } from "./AgentWorkbenchPane";

const CONVERSATION_PANEL_ID = "conversation";
const SIDE_PANEL_ID = "side";

export interface AgentWorkbenchLayoutProps extends AgentWorkbenchPaneProps {
  conversation: ReactNode;
}

export function AgentWorkbenchLayout({
  conversation,
  proposals,
  proposalLoading,
  onOpenProposal,
}: AgentWorkbenchLayoutProps) {
  const panelSizes = useDesktopUiStore((state) => state.agentWorkbenchPanelSizes);
  const setPanelSizes = useDesktopUiStore((state) => state.setAgentWorkbenchPanelSizes);

  return (
    <Group
      id="agent-workbench"
      className="agent-workbench"
      orientation="horizontal"
      defaultLayout={{
        [CONVERSATION_PANEL_ID]: panelSizes.conversation,
        [SIDE_PANEL_ID]: panelSizes.side,
      }}
      onLayoutChanged={(layout) => {
        const conversation = layout[CONVERSATION_PANEL_ID];
        const side = layout[SIDE_PANEL_ID];
        if (conversation == null || side == null) {
          return;
        }
        setPanelSizes({ conversation, side });
      }}
    >
      <Panel
        id={CONVERSATION_PANEL_ID}
        className="agent-workbench-conversation"
        minSize="35%"
        defaultSize={`${panelSizes.conversation}%`}
      >
        {conversation}
      </Panel>
      <Separator className="agent-workbench-resize-handle" />
      <Panel
        id={SIDE_PANEL_ID}
        className="agent-workbench-side"
        minSize="25%"
        defaultSize={`${panelSizes.side}%`}
      >
        <AgentWorkbenchPane
          proposals={proposals}
          proposalLoading={proposalLoading}
          onOpenProposal={onOpenProposal}
        />
      </Panel>
    </Group>
  );
}
