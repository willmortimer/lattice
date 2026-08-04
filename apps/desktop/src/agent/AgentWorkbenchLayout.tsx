import { Group, Panel, Separator } from "react-resizable-panels";
import type { ReactNode } from "react";

import { ProposalReviewBody } from "../ProposalReviewBody";
import type { TransactionProposal } from "../lib/proposals";
import { useDesktopUiStore } from "../shell/desktopUiStore";
import { AgentWorkbenchPane, type AgentWorkbenchPaneProps } from "./AgentWorkbenchPane";

const CONVERSATION_PANEL_ID = "conversation";
const SIDE_PANEL_ID = "side";

export interface AgentWorkbenchLayoutProps extends AgentWorkbenchPaneProps {
  conversation: ReactNode;
  proposalReview?: TransactionProposal | null;
  proposalReviewBusy?: boolean;
  workspaceRoot?: string | null;
  onProposalAccept?: (selectedCommandIndices: number[]) => void | Promise<void>;
  onProposalReject?: () => void | Promise<void>;
  onProposalCancel?: () => void;
}

export function AgentWorkbenchLayout({
  conversation,
  proposals,
  proposalLoading,
  onOpenProposal,
  proposalReview = null,
  proposalReviewBusy = false,
  workspaceRoot = null,
  threadId = null,
  onProposalAccept,
  onProposalReject,
  onProposalCancel,
}: AgentWorkbenchLayoutProps) {
  const panelSizes = useDesktopUiStore((state) => state.agentWorkbenchPanelSizes);
  const setPanelSizes = useDesktopUiStore((state) => state.setAgentWorkbenchPanelSizes);

  const showProposalSplit =
    Boolean(proposalReview) &&
    Boolean(workspaceRoot) &&
    Boolean(onProposalAccept) &&
    Boolean(onProposalReject);

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
        const nextConversation = layout[CONVERSATION_PANEL_ID];
        const nextSide = layout[SIDE_PANEL_ID];
        if (nextConversation == null || nextSide == null) {
          return;
        }
        setPanelSizes({ conversation: nextConversation, side: nextSide });
      }}
    >
      <Panel
        id={CONVERSATION_PANEL_ID}
        className="agent-workbench-conversation"
        minSize="35%"
        defaultSize={`${panelSizes.conversation}%`}
      >
        {showProposalSplit && proposalReview && workspaceRoot ? (
          <ProposalReviewBody
            proposal={proposalReview}
            workspaceRoot={workspaceRoot}
            busy={proposalReviewBusy}
            embedded
            onAccept={onProposalAccept!}
            onReject={onProposalReject!}
            onCancel={onProposalCancel}
          />
        ) : (
          conversation
        )}
      </Panel>
      <Separator className="agent-workbench-resize-handle" />
      <Panel
        id={SIDE_PANEL_ID}
        className="agent-workbench-side"
        minSize="25%"
        defaultSize={`${panelSizes.side}%`}
      >
        <AgentWorkbenchPane
          workspaceRoot={workspaceRoot}
          threadId={threadId}
          proposals={proposals}
          proposalLoading={proposalLoading}
          onOpenProposal={onOpenProposal}
          activeProposalId={proposalReview?.id ?? null}
        />
      </Panel>
    </Group>
  );
}
