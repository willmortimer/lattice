import { describe, expect, it } from "vitest";

import type { RecentWorkspace } from "./profile";
import type { WorkspaceCatalog } from "./workspaceCatalog";
import {
  filterWorkspaceCatalogRows,
  groupWorkspaceCatalog,
  normalizeWorkspaceRoot,
  workspaceCatalogStatusLabel,
} from "./workspaceCatalogGroups";

const catalog: WorkspaceCatalog = {
  daemonReachable: true,
  via: "file",
  workspaces: [
    { workspaceId: "ws-default", root: "/Users/demo/Default", remoteAccessEnabled: false },
    { workspaceId: "ws-notes", root: "/Users/demo/Notes", remoteAccessEnabled: true },
    { workspaceId: "ws-archive", root: "/Users/demo/Archive", remoteAccessEnabled: false },
  ],
};

const recents: RecentWorkspace[] = [
  { root: "/Users/demo/Notes", title: "Notes", openedAt: 200 },
  { root: "/Users/demo/Archive", title: "Archive", openedAt: 100 },
  { root: "/Users/demo/Missing", title: "Gone", openedAt: 50 },
];

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
    expect(grouped.all).toHaveLength(3);
    expect(grouped.recent[0]?.status).toBe("remote");
    expect(workspaceCatalogStatusLabel("available")).toBe("Available");
  });

  it("filterWorkspaceCatalogRows matches title, path, and id", () => {
    const grouped = groupWorkspaceCatalog({ catalog, recents, pinnedRoot: null });
    expect(filterWorkspaceCatalogRows(grouped.all, "arch").map((row) => row.title)).toEqual([
      "Archive",
    ]);
    expect(filterWorkspaceCatalogRows(grouped.all, "ws-notes")).toHaveLength(1);
  });
});
