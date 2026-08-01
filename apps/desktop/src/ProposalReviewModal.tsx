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
import { useEffect, useMemo, useState } from "react";

import {
  defaultAcceptedCommandIndices,
  detailExcerptDisplay,
  hasHydrationProvenance,
  hydrationProvenanceLabel,
  previewCommandLabel,
  previewProposal,
  type CommandPreview,
  type CommandPreviewDetail,
  type DetailExcerptDisplay,
  type ProposalPreview,
  type TransactionProposal,
} from "./lib/proposals";

function ProposalDetailExcerpt({ detail }: { detail: CommandPreviewDetail | undefined }) {
  const display: DetailExcerptDisplay | null = detailExcerptDisplay(detail);
  if (!display) return null;
  if (display.mode === "plain") {
    return <pre className="proposal-command-excerpt">{display.text}</pre>;
  }
  if (display.before) {
    return (
      <div className="proposal-text-diff" role="group" aria-label="Text change preview">
        <pre className="proposal-command-excerpt proposal-diff-before" aria-label="Before">
          {display.before}
        </pre>
        <pre className="proposal-command-excerpt proposal-diff-after" aria-label="After">
          {display.after}
        </pre>
      </div>
    );
  }
  return <pre className="proposal-command-excerpt">{display.after}</pre>;
}

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
  const defaults = useMemo(() => defaultAcceptedCommandIndices(proposal), [proposal]);
  const [selected, setSelected] = useState<Set<number>>(() => new Set(defaults));
  const [preview, setPreview] = useState<ProposalPreview | null>(null);
  const [previewError, setPreviewError] = useState<string | null>(null);
  const [previewLoading, setPreviewLoading] = useState(false);

  const ordered = useMemo(() => [...selected].sort((a, b) => a - b), [selected]);
  const selectionKey = ordered.join(",");

  useEffect(() => {
    let cancelled = false;
    const indices = selectionKey
      ? selectionKey.split(",").map((value) => Number.parseInt(value, 10))
      : [];
    setPreviewLoading(true);
    setPreviewError(null);
    void previewProposal(workspaceRoot, proposal.id, indices)
      .then((next) => {
        if (cancelled) return;
        setPreview(next);
        setPreviewLoading(false);
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        setPreview(null);
        setPreviewError(String(err));
        setPreviewLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [workspaceRoot, proposal.id, selectionKey]);

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
  const previewByIndex = useMemo(() => {
    const map = new Map<number, CommandPreview>();
    for (const command of preview?.commands ?? []) {
      map.set(command.index, command);
    }
    return map;
  }, [preview]);

  const partialAccept = selected.size > 0 && selected.size < proposal.commands.length;
  const subsetValid = preview?.subsetValid ?? false;
  const acceptDisabled = busy || selected.size === 0 || previewLoading || !subsetValid;

  return (
    <DialogRoot open onOpenChange={(open) => !open && !busy && onCancel()}>
      <DialogPortal>
        <DialogBackdrop className="modal-backdrop" />
        <DialogPopup
          className="modal-panel proposal-review-panel"
          data-guidance-anchor="agent.proposal.review"
        >
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
          {hasHydrationProvenance(proposal.source) && (
            <div
              className="proposal-hydration-provenance"
              role="group"
              aria-label="Hydration input digests"
            >
              <p className="modal-copy">Hydration inputs:</p>
              <ul className="proposal-hydration-list">
                {proposal.source.hydrationInputs!.map((input) => (
                  <li key={`${input.path}:${input.contentHash}`}>
                    {hydrationProvenanceLabel(input)}
                    {input.resourceId ? ` · ${input.resourceId}` : ""}
                  </li>
                ))}
              </ul>
            </div>
          )}
          {partialAccept && (
            <p className="modal-copy proposal-partial-warn" role="status">
              Partial accept discards unselected commands — the whole proposal is dismissed after
              apply.
            </p>
          )}
          {previewError && (
            <p className="modal-copy proposal-subset-error" role="alert">
              Preview unavailable: {previewError}
            </p>
          )}
          {preview && !preview.subsetValid && preview.subsetErrors.length > 0 && (
            <p className="modal-copy proposal-subset-error" role="alert">
              {preview.subsetErrors.join(" · ")}
              {preview.missingPredecessors.length > 0
                ? ` (also select command${preview.missingPredecessors.length === 1 ? "" : "s"} ${preview.missingPredecessors
                    .map((index) => index + 1)
                    .join(", ")})`
                : ""}
            </p>
          )}
          <div className="proposal-command-list">
            {proposal.commands.map((command, index) => {
              const checked = selected.has(index);
              const commandPreview = previewByIndex.get(index);
              const needsPredecessor = preview?.missingPredecessors.includes(index) ?? false;
              const rowWarnings = commandPreview?.warnings ?? [];
              return (
                <label
                  key={index}
                  className={`proposal-command-row${needsPredecessor ? " is-required-predecessor" : ""}`}
                >
                  <CheckboxRoot
                    checked={checked}
                    disabled={busy}
                    onCheckedChange={() => toggle(index)}
                  >
                    <CheckboxIndicator />
                  </CheckboxRoot>
                  <span>
                    <strong>{previewCommandLabel(commandPreview, command, index)}</strong>
                    <small>
                      Command {index + 1}
                      {commandPreview?.commandType ? ` · ${commandPreview.commandType}` : ""}
                      {needsPredecessor ? " · required predecessor" : ""}
                    </small>
                    {commandPreview?.detail && (
                      <ProposalDetailExcerpt detail={commandPreview.detail} />
                    )}
                    {rowWarnings.map((warning) => (
                      <small key={warning} className="proposal-command-warning">
                        {warning}
                      </small>
                    ))}
                  </span>
                </label>
              );
            })}
          </div>
          <div className="modal-actions">
            <Button variant="ghost" disabled={busy} onClick={() => void onReject()}>
              Reject
            </Button>
            <Button disabled={acceptDisabled} onClick={() => void onAccept(ordered)}>
              Accept
            </Button>
          </div>
        </DialogPopup>
      </DialogPortal>
    </DialogRoot>
  );
}
