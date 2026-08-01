import type { ReactNode } from "react";

import type { TransactionProposal, TransactionProposalSummary } from "../lib/proposals";
import { useDesktopUiStore } from "../shell/desktopUiStore";
import { AgentTrail } from "./AgentTrail";
import { AgentWorkbenchLayout } from "./AgentWorkbenchLayout";

export interface AgentPanelBodyProps {
  thread: ReactNode;
  proposals?: readonly TransactionProposalSummary[];
  proposalLoading?: boolean;
  onOpenProposal?: (proposalId: string) => void | Promise<void>;
  proposalReview?: TransactionProposal | null;
  proposalReviewBusy?: boolean;
  workspaceRoot?: string | null;
  onProposalAccept?: (selectedCommandIndices: number[]) => void | Promise<void>;
  onProposalReject?: () => void | Promise<void>;
  onProposalCancel?: () => void;
}

export function AgentPanelBody({
  thread,
  proposals,
  proposalLoading,
  onOpenProposal,
  proposalReview = null,
  proposalReviewBusy = false,
  workspaceRoot = null,
  onProposalAccept,
  onProposalReject,
  onProposalCancel,
}: AgentPanelBodyProps) {
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
      return (
        <AgentWorkbenchLayout
          conversation={thread}
          proposals={proposals}
          proposalLoading={proposalLoading}
          onOpenProposal={onOpenProposal}
          proposalReview={proposalReview}
          proposalReviewBusy={proposalReviewBusy}
          workspaceRoot={workspaceRoot}
          onProposalAccept={onProposalAccept}
          onProposalReject={onProposalReject}
          onProposalCancel={onProposalCancel}
        />
      );
    default: {
      const _exhaustive: never = layoutMode;
      return _exhaustive;
    }
  }
}
