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

import type { LinkRepairCandidate, LinkRepairPathChange, LinkRepairPlan } from "./lib/linkRepair";
import { defaultAcceptedCandidateIds } from "./lib/linkRepair";

export interface LinkRepairReviewModalProps {
  plan: LinkRepairPlan;
  mode: "lattice-rename" | "external";
  moves?: readonly LinkRepairPathChange[];
  busy?: boolean;
  truncated?: boolean;
  omittedCoMovedCount?: number;
  warnLargeRepairSet?: boolean;
  onAccept: (acceptedCandidateIds: string[]) => void | Promise<void>;
  onDefer: () => void | Promise<void>;
}

function candidateLabel(candidate: LinkRepairCandidate): string {
  const source = candidate.occurrence.sourcePath.split("/").pop() ?? candidate.occurrence.sourcePath;
  return `${source}: ${candidate.oldTarget} → ${candidate.newTarget}`;
}

export function LinkRepairReviewModal({
  plan,
  mode,
  moves,
  busy = false,
  truncated = false,
  omittedCoMovedCount = 0,
  warnLargeRepairSet = false,
  onAccept,
  onDefer,
}: LinkRepairReviewModalProps) {
  const resolvedDefaults = useMemo(() => defaultAcceptedCandidateIds(plan), [plan]);
  const [selected, setSelected] = useState<Set<string>>(() => new Set(resolvedDefaults));
  const isBatch = (moves?.length ?? 0) > 1;

  const toggle = (id: string) => {
    setSelected((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const ambiguousCount = plan.candidates.filter((candidate) => candidate.status === "ambiguous").length;
  const pathSummary = isBatch
    ? `Moving ${moves!.length} resources. Select which link references to rewrite.`
    : `${plan.renameFrom} → ${plan.renameTo}. Select which link references to rewrite.`;

  return (
    <DialogRoot open onOpenChange={(open) => !open && !busy && void onDefer()}>
      <DialogPortal>
        <DialogBackdrop className="modal-backdrop" />
        <DialogPopup className="modal-panel link-repair-panel">
          <DialogTitle id="link-repair-title">
            {mode === "lattice-rename"
              ? isBatch
                ? "Update links for batch move?"
                : "Update links for path change?"
              : "Repair links after external rename"}
          </DialogTitle>
          <p className="modal-copy">
            {pathSummary}
            {ambiguousCount > 0 ? ` ${ambiguousCount} ambiguous link(s) need manual review.` : ""}
            {omittedCoMovedCount > 0
              ? ` ${omittedCoMovedCount} link(s) inside co-moved pages were skipped (same-transaction path conflict).`
              : ""}
            {truncated
              ? " Candidate list was truncated to the batch hard cap (500); remaining links stay as-is."
              : ""}
            {warnLargeRepairSet && !truncated
              ? " This repair set is large (200+ candidates); review carefully before accepting."
              : ""}
          </p>
          <div className="link-repair-list">
            {plan.candidates.map((candidate) => {
              const disabled = candidate.status !== "resolved";
              const checked = selected.has(candidate.id);
              return (
                <label key={candidate.id} className={`link-repair-row${disabled ? " is-disabled" : ""}`}>
                  <CheckboxRoot
                    checked={checked}
                    disabled={disabled || busy}
                    onCheckedChange={() => !disabled && toggle(candidate.id)}
                  >
                    <CheckboxIndicator />
                  </CheckboxRoot>
                  <span>
                    <strong>{candidateLabel(candidate)}</strong>
                    <small>
                      {candidate.status === "ambiguous"
                        ? `Ambiguous (${candidate.ambiguity?.length ?? 0} matches)`
                        : candidate.newText}
                    </small>
                  </span>
                </label>
              );
            })}
          </div>
          <div className="modal-actions">
            <Button variant="ghost" disabled={busy} onClick={() => void onDefer()}>
              Defer
            </Button>
            <Button
              disabled={busy || selected.size === 0}
              onClick={() => void onAccept([...selected])}
            >
              {mode === "lattice-rename"
                ? isBatch
                  ? "Move and repair"
                  : "Rename and repair"
                : "Apply repairs"}
            </Button>
          </div>
        </DialogPopup>
      </DialogPortal>
    </DialogRoot>
  );
}
