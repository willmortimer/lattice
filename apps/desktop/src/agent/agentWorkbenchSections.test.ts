import { describe, expect, it } from "vitest";

import type { TrailStep } from "./agentStore";
import {
  approvalTrailSteps,
  changeTrailSteps,
  formatWorkbenchTrailKind,
  pendingApprovalProposals,
  planTrailSteps,
  trailStepKey,
  workbenchTrailDetail,
} from "./agentWorkbenchSections";

function trailStep(
  overrides: Partial<TrailStep> & Pick<TrailStep, "stepId" | "runId" | "kind" | "label">,
): TrailStep {
  return {
    status: "completed",
    ...overrides,
  };
}

describe("agentWorkbenchSections", () => {
  it("partitions trail steps into plan, change, and approval buckets", () => {
    const steps: TrailStep[] = [
      trailStep({
        runId: "run-1",
        stepId: "s1",
        kind: "model",
        label: "Reason about task",
        summary: "Outline edits",
      }),
      trailStep({
        runId: "run-1",
        stepId: "s2",
        kind: "proposal",
        label: "Create page proposal",
      }),
      trailStep({
        runId: "run-1",
        stepId: "s3",
        kind: "validation",
        label: "Await approval",
        status: "in_progress",
      }),
      trailStep({
        runId: "run-1",
        stepId: "s4",
        kind: "tool",
        label: "Search workspace",
      }),
    ];

    expect(planTrailSteps(steps).map((step) => step.stepId)).toEqual(["s1"]);
    expect(changeTrailSteps(steps).map((step) => step.stepId)).toEqual(["s2"]);
    expect(approvalTrailSteps(steps).map((step) => step.stepId)).toEqual(["s3"]);
  });

  it("formats trail keys and detail labels", () => {
    const step = trailStep({
      runId: "run-1",
      stepId: "s1",
      kind: "model",
      label: "Plan",
      summary: "Step summary",
    });

    expect(trailStepKey(step)).toBe("run-1:s1");
    expect(formatWorkbenchTrailKind("model")).toBe("model");
    expect(workbenchTrailDetail(step)).toBe("Step summary");
    expect(workbenchTrailDetail({ ...step, summary: undefined })).toBe("Plan");
  });

  it("filters pending approval proposals", () => {
    const proposals = [
      {
        id: "p1",
        status: "pending" as const,
        summary: "A",
        commandCount: 1,
        affectedPaths: [],
        warnings: [],
        createdAt: "2026-01-01T00:00:00Z",
        source: { type: "task" as const },
      },
      {
        id: "p2",
        status: "accepted" as const,
        summary: "B",
        commandCount: 1,
        affectedPaths: [],
        warnings: [],
        createdAt: "2026-01-01T00:00:00Z",
        source: { type: "task" as const },
      },
    ];

    expect(pendingApprovalProposals(proposals).map((item) => item.id)).toEqual(["p1"]);
  });
});
