import { describe, expect, it } from "vitest";

import type { CommandPreviewDetail, TransactionProposal } from "./executionContracts";
import {
  commandSummaryLabel,
  defaultAcceptedCommandIndices,
  detailExcerpt,
  previewCommandLabel,
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
