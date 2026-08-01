import { Button } from "@lattice/ui";
import type { ReactNode } from "react";

import type { TransactionProposalSummary } from "../lib/proposals";
import { useDesktopUiStore } from "../shell/desktopUiStore";
import { requestCloseDetachedAgent } from "./agentDetachedWindow";
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
  const setLayoutMode = useDesktopUiStore((state) => state.setAgentLayoutMode);

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
    case "detached":
      return (
        <div className="agent-detached-placeholder" role="status">
          <p>Agent is open in a separate window.</p>
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={() => {
              setLayoutMode("dock");
              void requestCloseDetachedAgent();
            }}
          >
            Return to Dock
          </Button>
        </div>
      );
    default: {
      const _exhaustive: never = layoutMode;
      return _exhaustive;
    }
  }
}
