import { describe, expect, it } from "vitest";

import type { CommandPreviewDetail, TransactionProposal } from "./executionContracts";
import {
  commandSummaryLabel,
  compareSidesFromDetail,
  defaultAcceptedCommandIndices,
  detailExcerpt,
  detailExcerptDisplay,
  filterProposalSummaries,
  formatTextDiffExcerpt,
  hasHydrationProvenance,
  hydrationProvenanceLabel,
  pathsFromSelectedCommands,
  previewCommandLabel,
  proposalCompareSections,
  proposalStatusLabel,
  shortContentHash,
} from "./proposals";

const sampleProposal: TransactionProposal = {
  id: "prop-1",
  source: { type: "task", resource: "tasks/demo.task" },
  summary: "Create notes",
  commands: [
    { type: "page-create", path: "Notes/A.md", content: "# A" },
    { type: "page-create", path: "Notes/B.md", content: "# B" },
    { type: "resource-rename", from: "Notes/A.md", to: "Notes/C.md" },
  ],
  affectedPaths: ["Notes/A.md", "Notes/B.md", "Notes/C.md"],
  warnings: [],
  createdAt: "2026-07-21T17:00:00Z",
};

/** Typed fixture matching daemon `hydrationInputs` wire shape (camelCase). */
export const PROPOSAL_WITH_HYDRATION_FIXTURE: TransactionProposal = {
  id: "prop-hydration",
  source: {
    type: "mcp",
    resource: "wasi://run_1/guest.wasm",
    hydrationInputs: [
      {
        path: "hello.txt",
        contentHash:
          "0f328ae687eb8fd2acfa3a910bb6722eff43f8a7dbd08e53e572ae37a0c5d7a5",
        resourceId: "res-1",
      },
    ],
  },
  summary: "WASI guest output",
  commands: [],
  affectedPaths: ["Pages/Output.md"],
  warnings: [],
  createdAt: "2026-07-21T16:30:00Z",
};

export const PROPOSAL_WITHOUT_HYDRATION_FIXTURE: TransactionProposal = {
  ...PROPOSAL_WITH_HYDRATION_FIXTURE,
  id: "prop-plain",
  source: { type: "external" },
};

describe("defaultAcceptedCommandIndices", () => {
  it("selects every command index by default", () => {
    expect(defaultAcceptedCommandIndices(sampleProposal)).toEqual([0, 1, 2]);
  });

  it("returns empty for an empty proposal", () => {
    expect(defaultAcceptedCommandIndices({ ...sampleProposal, commands: [] })).toEqual([]);
  });
});

describe("commandSummaryLabel", () => {
  it("uses type and path when present", () => {
    expect(commandSummaryLabel(sampleProposal.commands[0], 0)).toBe("page-create: Notes/A.md");
  });

  it("falls back to from for rename-shaped commands", () => {
    expect(commandSummaryLabel(sampleProposal.commands[2], 2)).toBe("resource-rename: Notes/A.md");
  });

  it("labels unknown payloads by index", () => {
    expect(commandSummaryLabel(null, 4)).toBe("Command 5");
  });
});

describe("previewCommandLabel", () => {
  it("prefers backend preview summary", () => {
    expect(
      previewCommandLabel(
        {
          index: 0,
          commandType: "resource-create",
          summary: "Create interface Agent digest (1 component)",
          touchedPaths: ["CRM.data/interfaces/AgentDigest.interface.yaml"],
          warnings: [],
        },
        sampleProposal.commands[0],
        0,
      ),
    ).toBe("Create interface Agent digest (1 component)");
  });
});

describe("detailExcerpt", () => {
  it("returns text and summary excerpts", () => {
    const textCreate: CommandPreviewDetail = {
      kind: "text-create",
      path: "Notes/A.md",
      contentExcerpt: "# A",
      truncated: false,
      byteLen: 3,
    };
    expect(detailExcerpt(textCreate)).toBe("# A");
    expect(
      detailExcerpt({
        kind: "interface-summary",
        path: "x.interface.yaml",
        excerpt: "format: lattice-interface",
        truncated: false,
      }),
    ).toBe("format: lattice-interface");
    expect(detailExcerpt(undefined)).toBeNull();
  });
});

describe("formatTextDiffExcerpt", () => {
  it("returns after-only when before is missing or blank", () => {
    expect(formatTextDiffExcerpt(undefined, "# After")).toBe("# After");
    expect(formatTextDiffExcerpt("   ", "# After")).toBe("# After");
  });

  it("returns after when before and after are identical", () => {
    expect(formatTextDiffExcerpt("# Same", "# Same")).toBe("# Same");
  });

  it("formats a compact unified hunk when both differ", () => {
    expect(formatTextDiffExcerpt("line one\nline two", "line one\nline three")).toBe(
      "- line one\n- line two\n+ line one\n+ line three",
    );
  });
});

describe("detailExcerptDisplay", () => {
  it("surfaces before and after for text-diff details", () => {
    expect(
      detailExcerptDisplay({
        kind: "text-diff",
        path: "Notes/A.md",
        beforeExcerpt: "# Before",
        afterExcerpt: "# After",
        truncated: false,
      }),
    ).toEqual({ mode: "diff", before: "# Before", after: "# After" });
  });

  it("omits blank before excerpts", () => {
    expect(
      detailExcerptDisplay({
        kind: "text-diff",
        path: "Notes/A.md",
        beforeExcerpt: "  ",
        afterExcerpt: "# After",
        truncated: false,
      }),
    ).toEqual({ mode: "diff", before: null, after: "# After" });
  });
});

describe("detailExcerpt text-diff", () => {
  it("uses unified diff formatting when before and after differ", () => {
    expect(
      detailExcerpt({
        kind: "text-diff",
        path: "Notes/A.md",
        beforeExcerpt: "old",
        afterExcerpt: "new",
        truncated: false,
      }),
    ).toBe("- old\n+ new");
  });
});

describe("compareSidesFromDetail", () => {
  it("maps text-diff to current and proposed", () => {
    expect(
      compareSidesFromDetail({
        kind: "text-diff",
        path: "Notes/A.md",
        beforeExcerpt: "# Before",
        afterExcerpt: "# After",
        truncated: false,
      }),
    ).toEqual({
      path: "Notes/A.md",
      current: "# Before",
      proposed: "# After",
    });
  });

  it("treats creates as proposed-only", () => {
    expect(
      compareSidesFromDetail({
        kind: "text-create",
        path: "Notes/A.md",
        contentExcerpt: "# New",
        truncated: false,
        byteLen: 5,
      }),
    ).toEqual({
      path: "Notes/A.md",
      current: null,
      proposed: "# New",
    });
  });
});

describe("proposalCompareSections", () => {
  it("skips commands without detail and keeps labeled sides", () => {
    expect(
      proposalCompareSections([
        {
          index: 0,
          commandType: "page-create",
          summary: "Create A",
          touchedPaths: ["Notes/A.md"],
          warnings: [],
          detail: {
            kind: "text-create",
            path: "Notes/A.md",
            contentExcerpt: "# A",
            truncated: false,
            byteLen: 3,
          },
        },
        {
          index: 1,
          commandType: "noop",
          summary: "No detail",
          touchedPaths: [],
          warnings: [],
        },
      ]),
    ).toEqual([
      {
        path: "Notes/A.md",
        label: "Create A",
        current: null,
        proposed: "# A",
      },
    ]);
  });
});

describe("pathsFromSelectedCommands", () => {
  it("collects paths from selected commands", () => {
    expect(pathsFromSelectedCommands(sampleProposal, [0, 2])).toEqual([
      "Notes/A.md",
      "Notes/C.md",
    ]);
  });

  it("falls back to affected paths when commands lack paths", () => {
    expect(
      pathsFromSelectedCommands({ ...sampleProposal, commands: [null] }, [0]),
    ).toEqual(["Notes/A.md", "Notes/B.md", "Notes/C.md"]);
  });
});

describe("filterProposalSummaries", () => {
  const summaries = [
    {
      id: "1",
      source: { type: "task" as const, resource: "tasks/a.task" },
      summary: "Create Notes/A.md",
      commandCount: 1,
      affectedPaths: ["Notes/A.md"],
      warnings: [],
      createdAt: "2026-07-21T17:00:00Z",
      status: "pending" as const,
    },
    {
      id: "2",
      source: { type: "external" as const },
      summary: "Rejected page",
      commandCount: 1,
      affectedPaths: ["Notes/B.md"],
      warnings: [],
      createdAt: "2026-07-21T17:01:00Z",
      status: "rejected" as const,
    },
  ];

  it("filters by status, source, and path query", () => {
    expect(
      filterProposalSummaries(summaries, {
        status: "pending",
        source: "all",
        pathQuery: "",
      }),
    ).toHaveLength(1);
    expect(
      filterProposalSummaries(summaries, {
        status: "all",
        source: "task",
        pathQuery: "",
      })[0]?.id,
    ).toBe("1");
    expect(
      filterProposalSummaries(summaries, {
        status: "all",
        source: "all",
        pathQuery: "notes/b",
      }),
    ).toHaveLength(1);
  });
});

describe("proposalStatusLabel", () => {
  it("labels known statuses", () => {
    expect(proposalStatusLabel("pending")).toBe("Pending");
    expect(proposalStatusLabel("accepted")).toBe("Accepted");
  });
});

describe("hydration provenance helpers", () => {
  it("shortens long content hashes for display", () => {
    expect(shortContentHash("0f328ae687eb8fd2acfa3a910bb6722eff43f8a7dbd08e53e572ae37a0c5d7a5")).toBe(
      "0f328ae6…",
    );
    expect(shortContentHash("abc")).toBe("abc");
  });

  it("detects hydration inputs on proposal source", () => {
    expect(hasHydrationProvenance(PROPOSAL_WITH_HYDRATION_FIXTURE.source)).toBe(true);
    expect(hasHydrationProvenance(PROPOSAL_WITHOUT_HYDRATION_FIXTURE.source)).toBe(false);
    expect(hasHydrationProvenance({ type: "task", hydrationInputs: [] })).toBe(false);
  });

  it("formats path and short hash labels for review UI", () => {
    const [input] = PROPOSAL_WITH_HYDRATION_FIXTURE.source.hydrationInputs!;
    expect(hydrationProvenanceLabel(input)).toBe("hello.txt · 0f328ae6…");
  });

  it("gates provenance visibility the same way as ProposalReviewModal", () => {
    const showWith = hasHydrationProvenance(PROPOSAL_WITH_HYDRATION_FIXTURE.source);
    const showWithout = hasHydrationProvenance(PROPOSAL_WITHOUT_HYDRATION_FIXTURE.source);
    expect(showWith).toBe(true);
    expect(showWithout).toBe(false);
  });
});
