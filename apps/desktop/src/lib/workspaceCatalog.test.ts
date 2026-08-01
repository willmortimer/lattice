import { describe, expect, it } from "vitest";

import {
  emptyWorkspaceCatalog,
  workspaceCatalogDisplayName,
  type WorkspaceCatalogEntry,
} from "./workspaceCatalog";

describe("workspaceCatalog", () => {
  it("emptyWorkspaceCatalog returns a stable unavailable shape", () => {
    expect(emptyWorkspaceCatalog()).toEqual({
      workspaces: [],
      daemonReachable: false,
      via: "unavailable",
    });
  });

  it("workspaceCatalogDisplayName prefers the root leaf segment", () => {
    const entry: WorkspaceCatalogEntry = {
      workspaceId: "ws-1",
      root: "/Users/demo/Notes",
      remoteAccessEnabled: false,
    };
    expect(workspaceCatalogDisplayName(entry)).toBe("Notes");
  });
});
