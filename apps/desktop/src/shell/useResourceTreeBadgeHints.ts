import { useMemo } from "react";

import type { TransactionProposalSummary } from "../lib/executionContracts";
import {
  buildResourceTreeBadgeHints,
  type ResourceTreeBadgeHints,
} from "../lib/resourceTreeBadges";
import { useDesktopUiStore } from "./desktopUiStore";

export function useResourceTreeBadgeHints(
  proposalSummaries: readonly TransactionProposalSummary[],
  agentPanelOpen: boolean,
  selectedPath: string | null | undefined,
): ResourceTreeBadgeHints {
  const saveStatusBySessionId = useDesktopUiStore((state) => state.saveStatusBySessionId);
  return useMemo(
    () =>
      buildResourceTreeBadgeHints({
        saveStatusBySessionId,
        proposalSummaries,
        agentPanelOpen,
        selectedPath,
      }),
    [agentPanelOpen, proposalSummaries, saveStatusBySessionId, selectedPath],
  );
}
