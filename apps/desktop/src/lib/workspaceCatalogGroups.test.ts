import { describe, expect, it } from "vitest";

import type { RecentWorkspace } from "./profile";
import type { WorkspaceCatalog, WorkspaceSummary } from "./workspaceCatalog";
import {
  filterWorkspaceCatalogRows,
  groupWorkspaceCatalog,
  normalizeWorkspaceRoot,
  visibleWorkspaceCatalogIds,
  workspaceCatalogRowTitle,
  workspaceCatalogStatusLabel,
} from "./workspaceCatalogGroups";

const catalog: WorkspaceCatalog = {
  daemonReachable: true,
  via: "file",
  workspaces: [
    { workspaceId: "ws-default", root: "/Users/demo/Default", remoteAccessEnabled: false },
    { workspaceId: "ws-notes", root: "/Users/demo/Notes", remoteAccessEnabled: true },
    { workspaceId: "ws-archive", root: "/Users/demo/Archive", remoteAccessEnabled: false },
    { workspaceId: "ws-hidden", root: "/Users/demo/Hidden", remoteAccessEnabled: false },
  ],
};

const recents: RecentWorkspace[] = [
  { root: "/Users/demo/Notes", title: "Notes", openedAt: 200 },
  { root: "/Users/demo/Archive", title: "Archive", openedAt: 100 },
  { root: "/Users/demo/Missing", title: "Gone", openedAt: 50 },
];

function summary(
  workspaceId: string,
  title: string,
  overrides: Partial<WorkspaceSummary> = {},
): WorkspaceSummary {
  return {
    workspaceId,
    root: `/Users/demo/${workspaceId}`,
    title,
    remoteAccessEnabled: false,
    manifestPresent: true,
    via: "file",
    ...overrides,
  };
}

describe("workspaceCatalogGroups", () => {
  it("normalizeWorkspaceRoot strips trailing slashes", () => {
    expect(normalizeWorkspaceRoot("/Users/demo/Notes/")).toBe("/Users/demo/Notes");
  });

  it("groups pinned and recent from registry metadata without inventing unscanned roots", () => {
    const grouped = groupWorkspaceCatalog({
      catalog,
      recents,
      pinnedRoot: "/Users/demo/Default/",
    });

    expect(grouped.pinned.map((row) => row.entry.workspaceId)).toEqual(["ws-default"]);
    expect(grouped.recent.map((row) => row.entry.workspaceId)).toEqual(["ws-notes", "ws-archive"]);
    expect(grouped.all).toHaveLength(4);
    expect(grouped.recent[0]?.status).toBe("remote");
    expect(workspaceCatalogStatusLabel("available")).toBe("Available");
    expect(visibleWorkspaceCatalogIds(grouped)).toEqual(["ws-default", "ws-notes", "ws-archive"]);
  });

  it("prefers manifest summary titles over recents and path leaves", () => {
    const summaries = new Map<string, WorkspaceSummary>([
      ["ws-default", summary("ws-default", "Personal HQ")],
      ["ws-notes", summary("ws-notes", "Field Notes")],
      ["ws-hidden", summary("ws-hidden", "Should not appear on Home")],
    ]);
    const grouped = groupWorkspaceCatalog({
      catalog,
      recents,
      pinnedRoot: "/Users/demo/Default/",
      summaries,
    });

    expect(grouped.pinned[0]?.title).toBe("Personal HQ");
    expect(grouped.recent[0]?.title).toBe("Field Notes");
    expect(grouped.recent[1]?.title).toBe("Archive");
    expect(grouped.all.find((row) => row.entry.workspaceId === "ws-hidden")?.title).toBe(
      "Should not appear on Home",
    );
  });

  it("falls back to recents or path leaf when summary is missing or has no manifest", () => {
    const summaries = new Map<string, WorkspaceSummary>([
      ["ws-notes", summary("ws-notes", "Ignored Title", { manifestPresent: false })],
    ]);
    const grouped = groupWorkspaceCatalog({
      catalog,
      recents,
      pinnedRoot: "/Users/demo/Default/",
      summaries,
    });

    expect(grouped.pinned[0]?.title).toBe("Default");
    expect(grouped.recent[0]?.title).toBe("Notes");
    expect(grouped.recent[1]?.title).toBe("Archive");
    expect(
      workspaceCatalogRowTitle(catalog.workspaces[3]!, {
        summary: summary("ws-hidden", "  ", { manifestPresent: true }),
      }),
    ).toBe("Hidden");
  });

  it("visibleWorkspaceCatalogIds uses the registered list when pinned and recent are empty", () => {
    const grouped = groupWorkspaceCatalog({ catalog, recents: [], pinnedRoot: null });
    expect(visibleWorkspaceCatalogIds(grouped)).toEqual([
      "ws-default",
      "ws-notes",
      "ws-archive",
      "ws-hidden",
    ]);
  });

  it("filterWorkspaceCatalogRows matches title, path, and id", () => {
    const grouped = groupWorkspaceCatalog({ catalog, recents, pinnedRoot: null });
    expect(filterWorkspaceCatalogRows(grouped.all, "arch").map((row) => row.title)).toEqual([
      "Archive",
    ]);
    expect(filterWorkspaceCatalogRows(grouped.all, "ws-notes")).toHaveLength(1);
  });
});
