import { describe, expect, it } from "vitest";

import type { CommandPreviewDetail, TransactionProposal } from "./executionContracts";
import {
  commandSummaryLabel,
  defaultAcceptedCommandIndices,
  detailExcerpt,
  filterProposalSummaries,
  pathsFromSelectedCommands,
  previewCommandLabel,
  proposalStatusLabel,
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
