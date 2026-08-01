import { useCallback, useEffect, useRef, useState } from "react";

import { hasTauri } from "../lib/ipc";
import {
  applyProposal,
  dismissProposal,
  getProposal,
  listProposals,
  type TransactionProposal,
  type TransactionProposalSummary,
} from "../lib/proposals";

export type UseAgentProposalReviewResult = {
  proposalSummaries: TransactionProposalSummary[];
  proposalInboxLoading: boolean;
  proposalReview: TransactionProposal | null;
  proposalReviewBusy: boolean;
  refreshProposalInbox: () => Promise<void>;
  openProposalReview: (proposalId: string) => Promise<void>;
  handleProposalAccept: (selectedCommandIndices: number[]) => Promise<void>;
  handleProposalReject: () => Promise<void>;
  handleProposalCancel: () => void;
};

/**
 * Workspace-scoped proposal inbox + embedded review actions.
 * Mirrors the DesktopShell / useDesktopController wiring for detached workbench.
 */
export function useAgentProposalReview(
  workspaceRoot: string | null,
): UseAgentProposalReviewResult {
  const rootRef = useRef(workspaceRoot);
  rootRef.current = workspaceRoot;

  const [proposalSummaries, setProposalSummaries] = useState<TransactionProposalSummary[]>(
    [],
  );
  const [proposalInboxLoading, setProposalInboxLoading] = useState(false);
  const [proposalReview, setProposalReview] = useState<TransactionProposal | null>(null);
  const [proposalReviewBusy, setProposalReviewBusy] = useState(false);
  const proposalResolverRef = useRef<
    ((result: "accepted" | "rejected" | "cancelled") => void) | null
  >(null);

  const refreshProposalInbox = useCallback(async () => {
    const root = rootRef.current;
    if (!root || !hasTauri) {
      setProposalSummaries([]);
      return;
    }
    setProposalInboxLoading(true);
    try {
      setProposalSummaries(await listProposals(root));
    } catch {
      setProposalSummaries([]);
    } finally {
      setProposalInboxLoading(false);
    }
  }, []);

  useEffect(() => {
    if (!workspaceRoot || !hasTauri) {
      setProposalSummaries([]);
      return;
    }
    void refreshProposalInbox();
  }, [workspaceRoot, refreshProposalInbox]);

  const finishProposalReview = useCallback((result: "accepted" | "rejected" | "cancelled") => {
    proposalResolverRef.current?.(result);
    proposalResolverRef.current = null;
    setProposalReview(null);
  }, []);

  const openProposalReview = useCallback(async (proposalId: string) => {
    const root = rootRef.current;
    if (!root) {
      return;
    }
    try {
      const proposal = await getProposal(root, proposalId);
      await new Promise<"accepted" | "rejected" | "cancelled">((resolve) => {
        proposalResolverRef.current = resolve;
        setProposalReview(proposal);
      });
    } catch {
      // Detached has no shell error banner; leave review closed.
    }
  }, []);

  const handleProposalAccept = useCallback(
    async (selectedCommandIndices: number[]) => {
      const review = proposalReview;
      const root = rootRef.current;
      if (!review || !root) {
        return;
      }
      setProposalReviewBusy(true);
      try {
        await applyProposal(root, review.id, selectedCommandIndices);
        finishProposalReview("accepted");
        await refreshProposalInbox();
      } catch {
        finishProposalReview("cancelled");
      } finally {
        setProposalReviewBusy(false);
      }
    },
    [finishProposalReview, proposalReview, refreshProposalInbox],
  );

  const handleProposalReject = useCallback(async () => {
    const review = proposalReview;
    const root = rootRef.current;
    if (!review || !root) {
      return;
    }
    setProposalReviewBusy(true);
    try {
      await dismissProposal(root, review.id);
      finishProposalReview("rejected");
      await refreshProposalInbox();
    } catch {
      finishProposalReview("cancelled");
    } finally {
      setProposalReviewBusy(false);
    }
  }, [finishProposalReview, proposalReview, refreshProposalInbox]);

  const handleProposalCancel = useCallback(() => {
    finishProposalReview("cancelled");
  }, [finishProposalReview]);

  return {
    proposalSummaries,
    proposalInboxLoading,
    proposalReview,
    proposalReviewBusy,
    refreshProposalInbox,
    openProposalReview,
    handleProposalAccept,
    handleProposalReject,
    handleProposalCancel,
  };
}
