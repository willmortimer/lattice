import type { AgentStepKind } from "@lattice/agent-protocol";

import type { TransactionProposalSummary } from "../lib/proposals";
import type { TrailStep } from "./agentStore";

const PLAN_TRAIL_KINDS: readonly AgentStepKind[] = ["model"];

const CHANGE_TRAIL_KINDS: readonly AgentStepKind[] = ["proposal", "draft", "execution"];

const APPROVAL_TRAIL_KINDS: readonly AgentStepKind[] = ["validation"];

export function trailStepKey(step: Pick<TrailStep, "runId" | "stepId">): string {
  return `${step.runId}:${step.stepId}`;
}

export function formatWorkbenchTrailKind(kind: AgentStepKind): string {
  return kind.replace(/_/g, " ");
}

export function planTrailSteps(steps: readonly TrailStep[]): TrailStep[] {
  return steps.filter((step) => PLAN_TRAIL_KINDS.includes(step.kind));
}

export function changeTrailSteps(steps: readonly TrailStep[]): TrailStep[] {
  return steps.filter((step) => CHANGE_TRAIL_KINDS.includes(step.kind));
}

/** In-run validation pauses that still need a human decision. */
export function approvalTrailSteps(steps: readonly TrailStep[]): TrailStep[] {
  return steps.filter(
    (step) => APPROVAL_TRAIL_KINDS.includes(step.kind) && step.status === "in_progress",
  );
}

export function pendingApprovalProposals(
  proposals: readonly TransactionProposalSummary[],
): TransactionProposalSummary[] {
  return proposals.filter((item) => item.status === "pending");
}

export function workbenchTrailDetail(step: TrailStep): string {
  return step.summary ?? step.label;
}
