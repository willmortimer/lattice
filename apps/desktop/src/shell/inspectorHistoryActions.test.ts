import { beforeEach, describe, expect, it, vi } from "vitest";

const listResourceRevisions = vi.fn();
const getResourceRevision = vi.fn();
const revertResourceRevision = vi.fn();

vi.mock("../lib/revisions", () => ({
  listResourceRevisions: (...args: unknown[]) => listResourceRevisions(...args),
  getResourceRevision: (...args: unknown[]) => getResourceRevision(...args),
  revertResourceRevision: (...args: unknown[]) => revertResourceRevision(...args),
}));

import {
  formatRevisionDiff,
  guardedRevertResourceRevision,
  loadResourceHistory,
  loadResourceHistoryDetail,
  resolveExpectedCurrentRevision,
} from "./inspectorHistoryActions";

describe("inspector history actions", () => {
  beforeEach(() => {
    listResourceRevisions.mockReset();
    getResourceRevision.mockReset();
    revertResourceRevision.mockReset();
  });

  it("loads list, detail, and guarded revert with current baseline", async () => {
    const summaries = [
      {
        revisionId: "rev-2",
        resourcePath: "Notes/a.md",
        transactionId: null,
        summary: "edit",
        createdAt: 2,
        parentRevision: "rev-1",
        beforeHash: "b1",
        afterHash: "a2",
        beforeLen: 1,
        afterLen: 2,
        source: "local" as const,
        priorAvailable: true,
        pinned: false,
        currentBaseline: true,
        unresolvedConflict: false,
      },
      {
        revisionId: "rev-1",
        resourcePath: "Notes/a.md",
        transactionId: null,
        summary: "create",
        createdAt: 1,
        parentRevision: null,
        beforeHash: null,
        afterHash: "a1",
        beforeLen: null,
        afterLen: 1,
        source: "local" as const,
        priorAvailable: false,
        pinned: false,
        currentBaseline: false,
        unresolvedConflict: false,
      },
    ];
    listResourceRevisions.mockResolvedValue(summaries);
    getResourceRevision.mockResolvedValue({
      summary: summaries[1],
      base: null,
      local: null,
      incoming: null,
      diff: {
        isBinary: false,
        unified: "@@ -1 +1 @@\n-old\n+new\n",
        addedLines: 1,
        removedLines: 1,
        baseLen: 1,
        localLen: 1,
      },
      conflict: null,
    });
    revertResourceRevision.mockResolvedValue("rev-3");

    const listed = await loadResourceHistory("/ws", "Notes/a.md");
    expect(listResourceRevisions).toHaveBeenCalledWith("/ws", "Notes/a.md", 50);
    expect(listed).toHaveLength(2);

    const detail = await loadResourceHistoryDetail("/ws", "Notes/a.md", "rev-1");
    expect(getResourceRevision).toHaveBeenCalledWith("/ws", "Notes/a.md", "rev-1");
    expect(formatRevisionDiff(detail!)).toContain("@@ -1 +1 @@");

    const expected = resolveExpectedCurrentRevision(listed, null);
    expect(expected).toBe("rev-2");

    await expect(
      guardedRevertResourceRevision({
        root: "/ws",
        path: "Notes/a.md",
        revisionId: "rev-1",
        expectedCurrentRevision: expected,
      }),
    ).resolves.toBe("rev-3");
    expect(revertResourceRevision).toHaveBeenCalledWith("/ws", "Notes/a.md", "rev-1", "rev-2");
  });

  it("refuses revert without a known current revision and reports prior unavailable", async () => {
    await expect(
      guardedRevertResourceRevision({
        root: "/ws",
        path: "Notes/a.md",
        revisionId: "rev-1",
        expectedCurrentRevision: null,
      }),
    ).rejects.toThrow(/current revision is unknown/);

    expect(
      formatRevisionDiff({
        summary: {
          revisionId: "rev-1",
          resourcePath: "Notes/a.md",
          transactionId: null,
          summary: null,
          createdAt: 1,
          parentRevision: null,
          beforeHash: null,
          afterHash: "a1",
          beforeLen: null,
          afterLen: 1,
          source: "local",
          priorAvailable: false,
          pinned: false,
          currentBaseline: false,
          unresolvedConflict: false,
        },
        base: null,
        local: null,
        incoming: null,
        diff: {
          isBinary: false,
          unified: null,
          addedLines: 0,
          removedLines: 0,
          baseLen: null,
          localLen: null,
        },
        conflict: null,
      }),
    ).toBe("Prior content unavailable.");
  });
});
