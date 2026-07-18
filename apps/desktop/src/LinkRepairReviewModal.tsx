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

import type { LinkRepairCandidate, LinkRepairPlan } from "./lib/linkRepair";
import { defaultAcceptedCandidateIds } from "./lib/linkRepair";

export interface LinkRepairReviewModalProps {
  plan: LinkRepairPlan;
  mode: "lattice-rename" | "external";
  busy?: boolean;
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
  busy = false,
  onAccept,
  onDefer,
}: LinkRepairReviewModalProps) {
  const resolvedDefaults = useMemo(() => defaultAcceptedCandidateIds(plan), [plan]);
  const [selected, setSelected] = useState<Set<string>>(() => new Set(resolvedDefaults));

  const toggle = (id: string) => {
    setSelected((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const ambiguousCount = plan.candidates.filter((candidate) => candidate.status === "ambiguous").length;

  return (
    <DialogRoot open onOpenChange={(open) => !open && !busy && void onDefer()}>
      <DialogPortal>
        <DialogBackdrop className="modal-backdrop" />
        <DialogPopup className="modal-panel link-repair-panel">
          <DialogTitle id="link-repair-title">
            {mode === "lattice-rename" ? "Update links for path change?" : "Repair links after external rename"}
          </DialogTitle>
          <p className="modal-copy">
            {plan.renameFrom} → {plan.renameTo}. Select which link references to rewrite.
            {ambiguousCount > 0 ? ` ${ambiguousCount} ambiguous link(s) need manual review.` : ""}
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
              {mode === "lattice-rename" ? "Rename and repair" : "Apply repairs"}
            </Button>
          </div>
        </DialogPopup>
      </DialogPortal>
    </DialogRoot>
  );
}
