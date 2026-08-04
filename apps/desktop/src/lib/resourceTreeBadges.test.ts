import { describe, expect, it } from "vitest";

import { DIRTY_SAVE_STATE, IDLE_SAVE_STATE } from "../editor/saveState";
import type { TransactionProposalSummary } from "./executionContracts";
import {
  authorityBadgeForMode,
  buildResourceTreeBadgeHints,
  resourceTreeRowBadges,
  syncBadgeForPlannerOutcome,
  syncBadgesByPathFromReport,
} from "./resourceTreeBadges";
import type { WorkspaceSyncRunReport } from "./cloudSync";
import type { CatalogEntry } from "./resourceCatalog";

function pendingProposal(
  affectedPaths: string[],
  sourceType: TransactionProposalSummary["source"]["type"] = "task",
): TransactionProposalSummary {
  return {
    id: "proposal-1",
    source: { type: sourceType },
    summary: "Test proposal",
    commandCount: 1,
    affectedPaths,
    warnings: [],
    createdAt: "2026-01-01T00:00:00.000Z",
    status: "pending",
  };
}

describe("authorityBadgeForMode", () => {
  it("maps non-local authority modes to badge kinds", () => {
    expect(authorityBadgeForMode("cloud")).toBe("cloud");
    expect(authorityBadgeForMode("external")).toBe("external");
    expect(authorityBadgeForMode("immutable_import")).toBe("immutable");
    expect(authorityBadgeForMode("local")).toBeNull();
  });
});

describe("resourceTreeRowBadges", () => {
  it("returns badges in stable priority order", () => {
    const badges = resourceTreeRowBadges({
      resourceId: "uuid-a",
      path: "Notes/A.md",
      hints: {
        dirtyByPath: new Set(["Notes/A.md"]),
        proposalByPath: new Set(["Notes/A.md"]),
        agentByPath: new Set(["Notes/A.md"]),
        authorityByPath: { "Notes/A.md": "cloud" },
      },
    });

    expect(badges.map((badge) => badge.kind)).toEqual([
      "dirty",
      "proposal",
      "agent",
      "cloud",
    ]);
  });

  it("matches agent hints by resource id or path", () => {
    expect(
      resourceTreeRowBadges({
        resourceId: "uuid-a",
        path: "Notes/A.md",
        hints: { agentByResourceId: new Set(["uuid-a"]) },
      }).map((badge) => badge.kind),
    ).toEqual(["agent"]);

    expect(
      resourceTreeRowBadges({
        resourceId: "uuid-a",
        path: "Notes/A.md",
        hints: { agentByPath: new Set(["Notes/A.md"]) },
      }).map((badge) => badge.kind),
    ).toEqual(["agent"]);
  });
});

describe("buildResourceTreeBadgeHints", () => {
  it("collects dirty paths from unsaved renderer sessions", () => {
    const hints = buildResourceTreeBadgeHints({
      saveStatusBySessionId: {
        "a.md": DIRTY_SAVE_STATE,
        "b.md": IDLE_SAVE_STATE,
      },
      proposalSummaries: [],
    });

    expect([...hints.dirtyByPath ?? []]).toEqual(["a.md"]);
  });

  it("marks pending proposal paths and agent-sourced paths", () => {
    const hints = buildResourceTreeBadgeHints({
      saveStatusBySessionId: {},
      proposalSummaries: [
        pendingProposal(["agent.md"], "mcp"),
        pendingProposal(["workflow.md"], "workflow"),
        pendingProposal(["manual.md"], "artifact"),
      ],
    });

    expect([...hints.proposalByPath ?? []].sort()).toEqual([
      "agent.md",
      "manual.md",
      "workflow.md",
    ]);
    expect([...hints.agentByPath ?? []].sort()).toEqual(["agent.md", "workflow.md"]);
  });

  it("includes the selected path while the agent panel is open", () => {
    const hints = buildResourceTreeBadgeHints({
      saveStatusBySessionId: {},
      proposalSummaries: [],
      agentPanelOpen: true,
      selectedPath: "Focus.md",
    });

    expect([...hints.agentByPath ?? []]).toEqual(["Focus.md"]);
  });

  it("merges authority badges from shell cache", () => {
    const hints = buildResourceTreeBadgeHints({
      saveStatusBySessionId: {},
      proposalSummaries: [],
      authorityByPath: { "Notes/Cloud.md": "cloud", "Import/Ext.md": "external" },
    });

    expect(hints.authorityByPath).toEqual({
      "Notes/Cloud.md": "cloud",
      "Import/Ext.md": "external",
    });
  });

  it("merges sync conflict badges from shell cache", () => {
    const hints = buildResourceTreeBadgeHints({
      saveStatusBySessionId: {},
      proposalSummaries: [],
      syncByPath: { "Notes/Conflict.md": "syncConflict" },
    });

    expect(hints.syncByPath).toEqual({ "Notes/Conflict.md": "syncConflict" });
  });
});

describe("syncBadgeForPlannerOutcome", () => {
  it("maps conflicted planner rows to sync conflict badges", () => {
    expect(syncBadgeForPlannerOutcome("conflicted", "skipped_conflicted")).toBe("syncConflict");
    expect(syncBadgeForPlannerOutcome("dirty", "failed")).toBe("syncError");
    expect(syncBadgeForPlannerOutcome("in_sync", "no_op")).toBeNull();
  });
});

describe("syncBadgesByPathFromReport", () => {
  it("maps resource ids to catalog paths without overwriting silently", () => {
    const catalog = new Map<string, CatalogEntry>([
      [
        "uuid-conflict",
        {
          resourceId: "uuid-conflict",
          path: "Notes/Conflict.md",
          kind: "page",
          childCount: 0,
        },
      ],
    ]);

    const report: WorkspaceSyncRunReport = {
      cloudWorkspaceId: "cloud-ws-1",
      results: [
        {
          resourceId: "uuid-conflict",
          status: "conflicted",
          outcome: "skipped_conflicted",
        },
        {
          resourceId: "uuid-ok",
          status: "in_sync",
          outcome: "no_op",
        },
      ],
    };

    expect(syncBadgesByPathFromReport(report, catalog)).toEqual({
      "Notes/Conflict.md": "syncConflict",
    });
  });
});

describe("resourceTreeRowBadges sync priority", () => {
  it("shows sync conflict badges after dirty and before authority", () => {
    const badges = resourceTreeRowBadges({
      resourceId: "uuid-a",
      path: "Notes/A.md",
      hints: {
        dirtyByPath: new Set(["Notes/A.md"]),
        syncByPath: { "Notes/A.md": "syncConflict" },
        authorityByPath: { "Notes/A.md": "cloud" },
      },
    });

    expect(badges.map((badge) => badge.kind)).toEqual(["dirty", "syncConflict", "cloud"]);
  });
});
