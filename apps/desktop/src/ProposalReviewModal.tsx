import {
  DialogBackdrop,
  DialogPopup,
  DialogPortal,
  DialogRoot,
  DialogTitle,
} from "@lattice/ui";

import { ProposalReviewBody } from "./ProposalReviewBody";
import type { TransactionProposal } from "./lib/proposals";

export interface ProposalReviewModalProps {
  proposal: TransactionProposal;
  workspaceRoot: string;
  busy?: boolean;
  onAccept: (selectedCommandIndices: number[]) => void | Promise<void>;
  onReject: () => void | Promise<void>;
  onCancel: () => void;
}

export function ProposalReviewModal({
  proposal,
  workspaceRoot,
  busy = false,
  onAccept,
  onReject,
  onCancel,
}: ProposalReviewModalProps) {
  return (
    <DialogRoot open onOpenChange={(open) => !open && !busy && onCancel()}>
      <DialogPortal>
        <DialogBackdrop className="modal-backdrop" />
        <DialogPopup
          className="modal-panel proposal-review-panel"
          data-guidance-anchor="agent.proposal.review"
        >
          <DialogTitle id="proposal-review-title">Review proposed changes</DialogTitle>
          <ProposalReviewBody
            proposal={proposal}
            workspaceRoot={workspaceRoot}
            busy={busy}
            onAccept={onAccept}
            onReject={onReject}
            onCancel={onCancel}
          />
        </DialogPopup>
      </DialogPortal>
    </DialogRoot>
  );
}
