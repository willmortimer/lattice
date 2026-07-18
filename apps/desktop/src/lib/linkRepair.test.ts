import { describe, expect, it } from "vitest";

import {
  batchPlanAsLinkRepairPlan,
  batchWarnThresholdExceeded,
  defaultAcceptedCandidateIds,
  LINK_REPAIR_BATCH_CANDIDATE_HARD_CAP,
  LINK_REPAIR_BATCH_CANDIDATE_WARN_THRESHOLD,
  type BatchLinkRepairPlan,
  type LinkRepairPlan,
} from "./linkRepair";

describe("defaultAcceptedCandidateIds", () => {
  it("selects only resolved candidates", () => {
    const plan: LinkRepairPlan = {
      id: "p1",
      renameFrom: "Notes/A.md",
      renameTo: "Notes/B.md",
      source: "lattice-rename",
      createdAt: 1,
      candidates: [
        {
          id: "c1",
          occurrence: {
            sourcePath: "Notes/Home.md",
            kind: "wiki",
            rawTarget: "A",
            anchor: null,
            label: null,
            sourceStartByte: 0,
            sourceEndByte: 1,
            sourceStartLine: 1,
            sourceStartColumn: 1,
            sourceEndLine: 1,
            sourceEndColumn: 2,
          },
          oldTarget: "A",
          newTarget: "B",
          newText: "[[B]]",
          status: "resolved",
          ambiguity: null,
        },
        {
          id: "c2",
          occurrence: {
            sourcePath: "Notes/Home.md",
            kind: "wiki",
            rawTarget: "A",
            anchor: null,
            label: null,
            sourceStartByte: 2,
            sourceEndByte: 3,
            sourceStartLine: 1,
            sourceStartColumn: 3,
            sourceEndLine: 1,
            sourceEndColumn: 4,
          },
          oldTarget: "A",
          newTarget: "B",
          newText: "[[B]]",
          status: "ambiguous",
          ambiguity: [],
        },
      ],
    };
    expect(defaultAcceptedCandidateIds(plan)).toEqual(["c1"]);
  });
});

describe("batch link repair helpers", () => {
  const batchBase: BatchLinkRepairPlan = {
    id: "batch",
    moves: [
      { from: "Notes/A.md", to: "Archive/A.md" },
      { from: "Notes/B.md", to: "Archive/B.md" },
    ],
    source: "lattice-rename",
    createdAt: 1,
    candidates: [],
    omittedCoMovedCount: 0,
    truncated: false,
    candidateTotalBeforeCap: 0,
  };

  it("projects batch plan into single-plan shape for the review modal", () => {
    const projected = batchPlanAsLinkRepairPlan({
      ...batchBase,
      candidates: [
        {
          id: "batch-0",
          occurrence: {
            sourcePath: "Notes/Home.md",
            kind: "wiki",
            rawTarget: "A",
            anchor: null,
            label: null,
            sourceStartByte: 0,
            sourceEndByte: 5,
            sourceStartLine: 1,
            sourceStartColumn: 1,
            sourceEndLine: 1,
            sourceEndColumn: 6,
          },
          oldTarget: "A",
          newTarget: "A",
          newText: "[[A]]",
          status: "resolved",
          ambiguity: null,
        },
      ],
    });
    expect(projected.renameFrom).toBe("Notes/A.md");
    expect(projected.renameTo).toBe("Archive/A.md");
    expect(projected.candidates).toHaveLength(1);
  });

  it("warns at the documented soft threshold and on truncation", () => {
    expect(LINK_REPAIR_BATCH_CANDIDATE_WARN_THRESHOLD).toBe(200);
    expect(LINK_REPAIR_BATCH_CANDIDATE_HARD_CAP).toBe(500);
    expect(
      batchWarnThresholdExceeded({
        ...batchBase,
        candidateTotalBeforeCap: 199,
        truncated: false,
      }),
    ).toBe(false);
    expect(
      batchWarnThresholdExceeded({
        ...batchBase,
        candidateTotalBeforeCap: 200,
        truncated: false,
      }),
    ).toBe(true);
    expect(
      batchWarnThresholdExceeded({
        ...batchBase,
        candidateTotalBeforeCap: 10,
        truncated: true,
      }),
    ).toBe(true);
  });
});
