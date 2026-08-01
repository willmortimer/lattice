import type { ReactNode } from "react";

import type { TransactionProposalSummary } from "../lib/proposals";
import { useDesktopUiStore } from "../shell/desktopUiStore";
import { AgentTrail } from "./AgentTrail";
import { AgentWorkbenchLayout } from "./AgentWorkbenchLayout";

export interface AgentPanelBodyProps {
  thread: ReactNode;
  proposals?: readonly TransactionProposalSummary[];
  proposalLoading?: boolean;
  onOpenProposal?: (proposalId: string) => void | Promise<void>;
}

export function AgentPanelBody({
  thread,
  proposals,
  proposalLoading,
  onOpenProposal,
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
        />
      );
    default: {
      const _exhaustive: never = layoutMode;
      return _exhaustive;
    }
  }
}
