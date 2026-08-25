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
  formatEncryptedBackupOption,
  listEncryptedWorkspaceBackups,
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

  it("listEncryptedWorkspaceBackups rejects when not signed in", async () => {
    getCloudSessionStatus.mockResolvedValue({ signedIn: false });
    await expect(listEncryptedWorkspaceBackups("/ws", "ws-1")).rejects.toThrow(
      /Sign in under Settings/,
    );
    expect(invoke).not.toHaveBeenCalled();
  });

  it("listEncryptedWorkspaceBackups invokes without unlocking the DEK", async () => {
    getCloudSessionStatus.mockResolvedValue({ signedIn: true });
    invoke.mockResolvedValueOnce([
      {
        id: "bk-1",
        workspaceId: "cloud-ws-1",
        deviceId: null,
        size: 42,
        contentHash: "abc123",
        createdAt: 99,
      },
    ]);

    const list = await listEncryptedWorkspaceBackups("/ws", "ws-1");

    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke).toHaveBeenCalledWith("list_encrypted_workspace_backups_cmd", {
      root: "/ws",
    });
    expect(list).toHaveLength(1);
    expect(list[0]?.id).toBe("bk-1");
  });

  it("formatEncryptedBackupOption shows created time, size, and hash", () => {
    const label = formatEncryptedBackupOption({
      id: "bk-1",
      workspaceId: "cloud-ws-1",
      size: 2048,
      contentHash: "abcdef0123456789ffff",
      createdAt: 1_700_000_000,
    });
    expect(label).toContain("2.0 KB");
    expect(label).toContain("abcdef012345");
    expect(label).not.toContain("backups/");
  });
});
