import { beforeEach, describe, expect, it, vi } from "vitest";

const getCloudSessionStatus = vi.fn();

vi.mock("./cloud", () => ({
  getCloudSessionStatus: (...args: unknown[]) => getCloudSessionStatus(...args),
}));

const invoke = vi.fn();

vi.mock("./ipc", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));

import {
  putEncryptedWorkspaceBackup,
  restoreEncryptedWorkspaceBackup,
} from "./encryptedBackup";

describe("encryptedBackup", () => {
  beforeEach(() => {
    getCloudSessionStatus.mockReset();
    invoke.mockReset();
  });

  it("putEncryptedWorkspaceBackup rejects when not signed in", async () => {
    getCloudSessionStatus.mockResolvedValue({ signedIn: false });
    await expect(putEncryptedWorkspaceBackup("/ws", "ws-1")).rejects.toThrow(
      /Sign in under Settings/,
    );
    expect(invoke).not.toHaveBeenCalled();
  });

  it("restoreEncryptedWorkspaceBackup unlocks then invokes with optional backupId", async () => {
    getCloudSessionStatus.mockResolvedValue({ signedIn: true });
    invoke
      .mockResolvedValueOnce({ unlocked: true, workspaceId: "ws-1" })
      .mockResolvedValueOnce({
        backupId: "bk-1",
        restoredCount: 2,
        skipped: [{ path: "Notes.md", reason: "destination exists with different content" }],
      });

    const result = await restoreEncryptedWorkspaceBackup(
      "/ws",
      "/restore-to",
      "ws-1",
      "bk-1",
    );

    expect(invoke).toHaveBeenNthCalledWith(1, "workspace_crypto_unlock_cmd", {
      workspaceId: "ws-1",
    });
    expect(invoke).toHaveBeenNthCalledWith(2, "restore_encrypted_workspace_backup_cmd", {
      root: "/ws",
      targetRoot: "/restore-to",
      backupId: "bk-1",
    });
    expect(result.backupId).toBe("bk-1");
    expect(result.restoredCount).toBe(2);
    expect(result.skipped).toHaveLength(1);
  });

  it("restoreEncryptedWorkspaceBackup rejects when not signed in", async () => {
    getCloudSessionStatus.mockResolvedValue({ signedIn: false });
    await expect(
      restoreEncryptedWorkspaceBackup("/ws", "/out", "ws-1"),
    ).rejects.toThrow(/Sign in under Settings/);
    expect(invoke).not.toHaveBeenCalled();
  });
});
