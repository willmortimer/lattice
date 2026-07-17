import { describe, expect, it } from "vitest";

import { defaultAcceptedCandidateIds, type LinkRepairPlan } from "./linkRepair";

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
