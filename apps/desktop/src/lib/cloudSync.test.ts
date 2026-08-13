import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../demo", () => ({
  inBrowser: false,
}));

const invokeMock = vi.fn();

vi.mock("./ipc", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

import {
  conflictedResourceIds,
  resolveWorkspaceSyncConflict,
  type WorkspaceSyncRunReport,
} from "./cloudSync";

describe("resolveWorkspaceSyncConflict", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("invokes resolve_workspace_sync_conflict_cmd with keep_local", async () => {
    const result = {
      resourceId: "res-1",
      status: "in_sync",
      outcome: "kept_local",
      contentHash: "abc",
    };
    invokeMock.mockResolvedValue(result);
    await expect(
      resolveWorkspaceSyncConflict("/ws", "res-1", "keep_local"),
    ).resolves.toEqual(result);
    expect(invokeMock).toHaveBeenCalledWith("resolve_workspace_sync_conflict_cmd", {
      root: "/ws",
      resourceId: "res-1",
      resolution: "keep_local",
    });
  });

  it("invokes resolve_workspace_sync_conflict_cmd with take_cloud", async () => {
    invokeMock.mockResolvedValue({
      resourceId: "res-2",
      status: "in_sync",
      outcome: "took_cloud",
    });
    await resolveWorkspaceSyncConflict("/ws", "res-2", "take_cloud");
    expect(invokeMock).toHaveBeenCalledWith("resolve_workspace_sync_conflict_cmd", {
      root: "/ws",
      resourceId: "res-2",
      resolution: "take_cloud",
    });
  });
});

describe("conflictedResourceIds", () => {
  it("returns resource ids skipped as conflicted", () => {
    const report: WorkspaceSyncRunReport = {
      cloudWorkspaceId: "cloud-ws",
      results: [
        {
          resourceId: "a",
          status: "conflicted",
          outcome: "skipped_conflicted",
        },
        {
          resourceId: "b",
          status: "in_sync",
          outcome: "no_op",
        },
        {
          resourceId: "c",
          status: "dirty",
          outcome: "failed",
        },
      ],
    };
    expect(conflictedResourceIds(report)).toEqual(["a"]);
  });
});
