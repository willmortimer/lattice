import {
  Button,
  CheckboxIndicator,
  CheckboxRoot,
  DialogBackdrop,
  DialogPopup,
  DialogPortal,
  DialogRoot,
  DialogTitle,
} from "@lattice/ui";
import { useMemo, useState } from "react";

import {
  commandSummaryLabel,
  defaultAcceptedCommandIndices,
  type TransactionProposal,
} from "./lib/proposals";

export interface ProposalReviewModalProps {
  proposal: TransactionProposal;
  busy?: boolean;
  onAccept: (selectedCommandIndices: number[]) => void | Promise<void>;
  onReject: () => void | Promise<void>;
  onCancel: () => void;
}

export function ProposalReviewModal({
  proposal,
  busy = false,
  onAccept,
  onReject,
  onCancel,
}: ProposalReviewModalProps) {
  const defaults = useMemo(() => defaultAcceptedCommandIndices(proposal), [proposal]);
  const [selected, setSelected] = useState<Set<number>>(() => new Set(defaults));

  const toggle = (index: number) => {
    setSelected((current) => {
      const next = new Set(current);
      if (next.has(index)) next.delete(index);
      else next.add(index);
      return next;
    });
  };

  const sourceLabel = proposal.source.resource
    ? `${proposal.source.type} · ${proposal.source.resource}`
    : proposal.source.type;
  const ordered = [...selected].sort((a, b) => a - b);

  return (
    <DialogRoot open onOpenChange={(open) => !open && !busy && onCancel()}>
      <DialogPortal>
        <DialogBackdrop className="modal-backdrop" />
        <DialogPopup className="modal-panel proposal-review-panel">
          <DialogTitle id="proposal-review-title">Review proposed changes</DialogTitle>
          <p className="modal-copy">
            {proposal.summary}. Source: {sourceLabel}. Select which commands to apply in one
            transaction.
            {proposal.warnings.length > 0
              ? ` Warnings: ${proposal.warnings.join("; ")}.`
              : ""}
          </p>
          {proposal.affectedPaths.length > 0 && (
            <p className="modal-copy proposal-affected-paths">
              Affected: {proposal.affectedPaths.join(", ")}
            </p>
          )}
          <div className="proposal-command-list">
            {proposal.commands.map((command, index) => {
              const checked = selected.has(index);
              return (
                <label key={index} className="proposal-command-row">
                  <CheckboxRoot
                    checked={checked}
                    disabled={busy}
                    onCheckedChange={() => toggle(index)}
                  >
                    <CheckboxIndicator />
                  </CheckboxRoot>
                  <span>
                    <strong>{commandSummaryLabel(command, index)}</strong>
                    <small>Command {index + 1}</small>
                  </span>
                </label>
              );
            })}
          </div>
          <div className="modal-actions">
            <Button variant="ghost" disabled={busy} onClick={() => void onReject()}>
              Reject
            </Button>
            <Button
              disabled={busy || selected.size === 0}
              onClick={() => void onAccept(ordered)}
            >
              Accept
            </Button>
          </div>
        </DialogPopup>
      </DialogPortal>
    </DialogRoot>
  );
}
