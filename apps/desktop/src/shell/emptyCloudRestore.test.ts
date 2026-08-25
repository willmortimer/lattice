import { beforeEach, describe, expect, it, vi } from "vitest";

const listAccountCloudWorkspaces = vi.fn();
const listEncryptedBackupsForCloudWorkspace = vi.fn();
const restoreEncryptedBackupForCloudWorkspace = vi.fn();
const restoreEncryptedWorkspaceBackup = vi.fn();

vi.mock("../lib/encryptedBackup", () => ({
  listAccountCloudWorkspaces: (...args: unknown[]) => listAccountCloudWorkspaces(...args),
  listEncryptedBackupsForCloudWorkspace: (...args: unknown[]) =>
    listEncryptedBackupsForCloudWorkspace(...args),
  restoreEncryptedBackupForCloudWorkspace: (...args: unknown[]) =>
    restoreEncryptedBackupForCloudWorkspace(...args),
  restoreEncryptedWorkspaceBackup: (...args: unknown[]) =>
    restoreEncryptedWorkspaceBackup(...args),
}));

import {
  defaultEmptyRestoreCommands,
  EMPTY_RESTORE_ERRORS,
  emptyCloudRestorePhase,
  loadEmptyRestoreBackups,
  loadEmptyRestoreWorkspaces,
  nextSelectedId,
  runEmptyShellRestore,
  supportsNativeAppleSignIn,
  userSafeEmptyRestoreError,
  validateEmptyRestoreSelection,
} from "./emptyCloudRestore";

describe("emptyCloudRestore phase", () => {
  it("signed-out shows sign-in", () => {
    expect(
      emptyCloudRestorePhase({
        signedIn: false,
        workspacesLoading: true,
        selectedCloudWorkspaceId: "ws",
        backupsLoading: true,
        restoring: true,
      }),
    ).toBe("sign-in");
  });

  it("signed-in lists workspaces then backups", () => {
    expect(
      emptyCloudRestorePhase({
        signedIn: true,
        workspacesLoading: true,
        selectedCloudWorkspaceId: "",
        backupsLoading: false,
        restoring: false,
      }),
    ).toBe("loading-workspaces");
    expect(
      emptyCloudRestorePhase({
        signedIn: true,
        workspacesLoading: false,
        selectedCloudWorkspaceId: "",
        backupsLoading: false,
        restoring: false,
      }),
    ).toBe("pick-workspace");
    expect(
      emptyCloudRestorePhase({
        signedIn: true,
        workspacesLoading: false,
        selectedCloudWorkspaceId: "cloud-ws-1",
        backupsLoading: true,
        restoring: false,
      }),
    ).toBe("loading-backups");
    expect(
      emptyCloudRestorePhase({
        signedIn: true,
        workspacesLoading: false,
        selectedCloudWorkspaceId: "cloud-ws-1",
        backupsLoading: false,
        restoring: false,
      }),
    ).toBe("pick-backup");
    expect(
      emptyCloudRestorePhase({
        signedIn: true,
        workspacesLoading: false,
        selectedCloudWorkspaceId: "cloud-ws-1",
        backupsLoading: false,
        restoring: true,
      }),
    ).toBe("restoring");
  });
});

describe("emptyCloudRestore validation", () => {
  it("errors when destination or backup is missing", () => {
    expect(
      validateEmptyRestoreSelection({
        cloudWorkspaceId: "cloud-ws-1",
        backupId: "bk-1",
        targetRoot: null,
      }),
    ).toEqual({ ok: false, error: EMPTY_RESTORE_ERRORS.missingDestination });
    expect(
      validateEmptyRestoreSelection({
        cloudWorkspaceId: "cloud-ws-1",
        backupId: "  ",
        targetRoot: "/dest",
      }),
    ).toEqual({ ok: false, error: EMPTY_RESTORE_ERRORS.missingBackup });
    expect(
      validateEmptyRestoreSelection({
        cloudWorkspaceId: "",
        backupId: "bk-1",
        targetRoot: "/dest",
      }),
    ).toEqual({ ok: false, error: EMPTY_RESTORE_ERRORS.missingWorkspace });
  });
});

describe("emptyCloudRestore listing", () => {
  beforeEach(() => {
    listAccountCloudWorkspaces.mockReset();
    listEncryptedBackupsForCloudWorkspace.mockReset();
  });

  it("loads account workspaces via the cloud-id list command", async () => {
    listAccountCloudWorkspaces.mockResolvedValue([
      { id: "cloud-ws-1", name: "Notes", createdAt: 1 },
    ]);
    const loaded = await loadEmptyRestoreWorkspaces();
    expect(loaded).toEqual({
      ok: true,
      workspaces: [{ id: "cloud-ws-1", name: "Notes", createdAt: 1 }],
    });
    expect(listAccountCloudWorkspaces).toHaveBeenCalledTimes(1);
  });

  it("loads backups for the selected cloud workspace id", async () => {
    listEncryptedBackupsForCloudWorkspace.mockResolvedValue([
      {
        id: "bk-1",
        workspaceId: "cloud-ws-1",
        size: 12,
        contentHash: "abc",
        createdAt: 2,
      },
    ]);
    const loaded = await loadEmptyRestoreBackups("cloud-ws-1");
    expect(loaded.ok).toBe(true);
    expect(listEncryptedBackupsForCloudWorkspace).toHaveBeenCalledWith("cloud-ws-1");
  });

  it("keeps the previous selection when it is still present", () => {
    expect(nextSelectedId(["a", "b"], "b")).toBe("b");
    expect(nextSelectedId(["a", "b"], "gone")).toBe("a");
    expect(nextSelectedId([], "a")).toBe("");
  });
});

describe("runEmptyShellRestore", () => {
  beforeEach(() => {
    restoreEncryptedBackupForCloudWorkspace.mockReset();
    restoreEncryptedWorkspaceBackup.mockReset();
  });

  it("does not restore or open when destination is missing", async () => {
    const restore = vi.fn();
    const openWorkspace = vi.fn();
    const outcome = await runEmptyShellRestore(
      { cloudWorkspaceId: "cloud-ws-1", backupId: "bk-1", targetRoot: "" },
      { restore, openWorkspace },
    );
    expect(outcome).toEqual({ ok: false, error: EMPTY_RESTORE_ERRORS.missingDestination });
    expect(restore).not.toHaveBeenCalled();
    expect(openWorkspace).not.toHaveBeenCalled();
  });

  it("does not restore when backup is missing", async () => {
    const restore = vi.fn();
    const openWorkspace = vi.fn();
    const outcome = await runEmptyShellRestore(
      { cloudWorkspaceId: "cloud-ws-1", backupId: "", targetRoot: "/dest" },
      { restore, openWorkspace },
    );
    expect(outcome).toEqual({ ok: false, error: EMPTY_RESTORE_ERRORS.missingBackup });
    expect(restore).not.toHaveBeenCalled();
    expect(openWorkspace).not.toHaveBeenCalled();
  });

  it("invokes the cloud-id restore then opens the destination", async () => {
    const restore = vi.fn().mockResolvedValue({
      backupId: "bk-1",
      restoredCount: 4,
      skipped: [],
    });
    const openWorkspace = vi.fn().mockResolvedValue(undefined);
    const outcome = await runEmptyShellRestore(
      {
        cloudWorkspaceId: "cloud-ws-1",
        backupId: "bk-1",
        targetRoot: "/Users/me/Restored",
      },
      { restore, openWorkspace },
    );
    expect(restore).toHaveBeenCalledWith("cloud-ws-1", "/Users/me/Restored", "bk-1");
    expect(openWorkspace).toHaveBeenCalledWith("/Users/me/Restored");
    expect(restoreEncryptedWorkspaceBackup).not.toHaveBeenCalled();
    expect(outcome.ok).toBe(true);
  });

  it("default restore command is account-scoped, not open-workspace restore", async () => {
    restoreEncryptedBackupForCloudWorkspace.mockResolvedValue({
      backupId: "bk-1",
      restoredCount: 1,
      skipped: [],
    });
    const openWorkspace = vi.fn().mockResolvedValue(undefined);
    await runEmptyShellRestore(
      { cloudWorkspaceId: "cloud-ws-1", backupId: "bk-1", targetRoot: "/dest" },
      { restore: defaultEmptyRestoreCommands.restore, openWorkspace },
    );
    expect(restoreEncryptedBackupForCloudWorkspace).toHaveBeenCalledWith(
      "cloud-ws-1",
      "/dest",
      "bk-1",
    );
    expect(restoreEncryptedWorkspaceBackup).not.toHaveBeenCalled();
    expect(openWorkspace).toHaveBeenCalledWith("/dest");
  });

  it("maps wrap-key failures from restore without dumping internals", async () => {
    const restore = vi.fn().mockRejectedValue(
      new Error("cloud API error (404): GET /v1/me/backup-wrap-key"),
    );
    const openWorkspace = vi.fn();
    const outcome = await runEmptyShellRestore(
      { cloudWorkspaceId: "cloud-ws-1", backupId: "bk-1", targetRoot: "/dest" },
      { restore, openWorkspace },
    );
    expect(outcome).toEqual({ ok: false, error: EMPTY_RESTORE_ERRORS.wrapKeyUnavailable });
    expect(openWorkspace).not.toHaveBeenCalled();
  });

  it("does not claim success when opening the restored folder fails", async () => {
    const restore = vi.fn().mockResolvedValue({
      backupId: "bk-1",
      restoredCount: 1,
      skipped: [],
    });
    const openWorkspace = vi.fn().mockRejectedValue(new Error("not a lattice workspace"));
    const outcome = await runEmptyShellRestore(
      { cloudWorkspaceId: "cloud-ws-1", backupId: "bk-1", targetRoot: "/dest" },
      { restore, openWorkspace },
    );
    expect(restore).toHaveBeenCalled();
    expect(openWorkspace).toHaveBeenCalledWith("/dest");
    expect(outcome.ok).toBe(false);
  });
});

describe("userSafeEmptyRestoreError", () => {
  it("maps wrap-key 404 without dumping internals", () => {
    expect(
      userSafeEmptyRestoreError(
        new Error("cloud API error (404): GET /v1/me/backup-wrap-key"),
      ),
    ).toBe(EMPTY_RESTORE_ERRORS.wrapKeyUnavailable);
    expect(userSafeEmptyRestoreError("wrap key not found (404)")).toBe(
      EMPTY_RESTORE_ERRORS.wrapKeyUnavailable,
    );
  });

  it("maps legacy backups to fail-closed copy", () => {
    expect(
      userSafeEmptyRestoreError(
        "account-scoped restore requires a wrapped backup envelope (LWBE); legacy backups need the original workspace DEK",
      ),
    ).toBe(EMPTY_RESTORE_ERRORS.legacyBackup);
  });

  it("does not leak unwrap internals", () => {
    expect(
      userSafeEmptyRestoreError(
        "failed to unwrap backup DEK (will not provision a new key): aead tag",
      ),
    ).toBe(EMPTY_RESTORE_ERRORS.unwrapFailed);
  });
});

describe("supportsNativeAppleSignIn", () => {
  it("is true on Apple platforms", () => {
    expect(supportsNativeAppleSignIn("MacIntel")).toBe(true);
    expect(supportsNativeAppleSignIn("Win32")).toBe(false);
  });
});
