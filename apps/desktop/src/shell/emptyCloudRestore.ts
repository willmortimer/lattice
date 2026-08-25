import {
  listAccountCloudWorkspaces,
  listEncryptedBackupsForCloudWorkspace,
  restoreEncryptedBackupForCloudWorkspace,
  type AccountCloudWorkspace,
  type EncryptedBackupListEntry,
  type EncryptedBackupRestoreResult,
} from "../lib/encryptedBackup";

export type EmptyCloudRestorePhase =
  | "sign-in"
  | "loading-workspaces"
  | "pick-workspace"
  | "loading-backups"
  | "pick-backup"
  | "restoring";

export const EMPTY_RESTORE_ERRORS = {
  missingWorkspace: "Choose a cloud workspace.",
  missingBackup: "Select a backup to restore.",
  missingDestination: "Choose a restore destination folder.",
  wrapKeyUnavailable:
    "Lattice Cloud cannot restore this backup yet. Try again after the cloud service is updated.",
  legacyBackup:
    "This backup cannot be restored on a new computer. Open the original workspace and restore from Settings.",
  unwrapFailed: "Could not unlock this backup. It may belong to a different Lattice Cloud account.",
  generic: "Could not restore the encrypted backup. Try again, or choose a different backup.",
} as const;

export interface EmptyRestoreSelection {
  cloudWorkspaceId: string;
  backupId: string;
  targetRoot: string | null | undefined;
}

export type EmptyRestoreValidation =
  | {
      ok: true;
      cloudWorkspaceId: string;
      backupId: string;
      targetRoot: string;
    }
  | { ok: false; error: string };

export type EmptyRestoreOutcome =
  | { ok: true; result: EncryptedBackupRestoreResult; targetRoot: string }
  | { ok: false; error: string };

export interface EmptyRestoreCommands {
  listWorkspaces: () => Promise<AccountCloudWorkspace[]>;
  listBackups: (cloudWorkspaceId: string) => Promise<EncryptedBackupListEntry[]>;
  restore: (
    cloudWorkspaceId: string,
    targetRoot: string,
    backupId: string,
  ) => Promise<EncryptedBackupRestoreResult>;
  openWorkspace: (path: string) => Promise<void>;
}

/** Account-scoped restore only — never the open-workspace legacy path. */
export const defaultEmptyRestoreCommands: Pick<
  EmptyRestoreCommands,
  "listWorkspaces" | "listBackups" | "restore"
> = {
  listWorkspaces: listAccountCloudWorkspaces,
  listBackups: listEncryptedBackupsForCloudWorkspace,
  restore: restoreEncryptedBackupForCloudWorkspace,
};

export function emptyCloudRestorePhase(input: {
  signedIn: boolean;
  workspacesLoading: boolean;
  selectedCloudWorkspaceId: string;
  backupsLoading: boolean;
  restoring: boolean;
}): EmptyCloudRestorePhase {
  if (!input.signedIn) return "sign-in";
  if (input.restoring) return "restoring";
  if (input.workspacesLoading) return "loading-workspaces";
  if (!input.selectedCloudWorkspaceId.trim()) return "pick-workspace";
  if (input.backupsLoading) return "loading-backups";
  return "pick-backup";
}

export function supportsNativeAppleSignIn(
  platform = typeof navigator !== "undefined" ? navigator.platform : "",
): boolean {
  return /Mac|iPhone|iPad/.test(platform);
}

export function validateEmptyRestoreSelection(
  input: EmptyRestoreSelection,
): EmptyRestoreValidation {
  const cloudWorkspaceId = input.cloudWorkspaceId.trim();
  if (!cloudWorkspaceId) {
    return { ok: false, error: EMPTY_RESTORE_ERRORS.missingWorkspace };
  }
  const backupId = input.backupId.trim();
  if (!backupId) {
    return { ok: false, error: EMPTY_RESTORE_ERRORS.missingBackup };
  }
  const targetRoot = input.targetRoot?.trim() ?? "";
  if (!targetRoot) {
    return { ok: false, error: EMPTY_RESTORE_ERRORS.missingDestination };
  }
  return { ok: true, cloudWorkspaceId, backupId, targetRoot };
}

function errorText(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

/** Map IPC/cloud failures to copy that does not dump wrap-key internals. */
export function userSafeEmptyRestoreError(err: unknown): string {
  const message = errorText(err);
  const lower = message.toLowerCase();
  if (lower.includes("legacy")) {
    return EMPTY_RESTORE_ERRORS.legacyBackup;
  }
  if (
    lower.includes("backup-wrap-key") ||
    (lower.includes("wrap key") && (lower.includes("404") || lower.includes("not found"))) ||
    (lower.includes("wrap") && lower.includes("cloud api error (404)"))
  ) {
    return EMPTY_RESTORE_ERRORS.wrapKeyUnavailable;
  }
  if (lower.includes("failed to unwrap") || lower.includes("will not provision")) {
    return EMPTY_RESTORE_ERRORS.unwrapFailed;
  }
  if (!message.trim()) {
    return EMPTY_RESTORE_ERRORS.generic;
  }
  if (lower.includes("sign in")) {
    return message;
  }
  if (
    lower.includes("choose a") ||
    lower.includes("is required") ||
    lower.includes("before restoring") ||
    lower.includes("before listing")
  ) {
    return message;
  }
  return EMPTY_RESTORE_ERRORS.generic;
}

export async function loadEmptyRestoreWorkspaces(
  listWorkspaces: EmptyRestoreCommands["listWorkspaces"] = defaultEmptyRestoreCommands.listWorkspaces,
): Promise<{ ok: true; workspaces: AccountCloudWorkspace[] } | { ok: false; error: string }> {
  try {
    const workspaces = await listWorkspaces();
    return { ok: true, workspaces };
  } catch (err: unknown) {
    return { ok: false, error: userSafeEmptyRestoreError(err) };
  }
}

export async function loadEmptyRestoreBackups(
  cloudWorkspaceId: string,
  listBackups: EmptyRestoreCommands["listBackups"] = defaultEmptyRestoreCommands.listBackups,
): Promise<{ ok: true; backups: EncryptedBackupListEntry[] } | { ok: false; error: string }> {
  const id = cloudWorkspaceId.trim();
  if (!id) {
    return { ok: false, error: EMPTY_RESTORE_ERRORS.missingWorkspace };
  }
  try {
    const backups = await listBackups(id);
    return { ok: true, backups };
  } catch (err: unknown) {
    return { ok: false, error: userSafeEmptyRestoreError(err) };
  }
}

export function nextSelectedId(ids: readonly string[], previous: string): string {
  return ids.some((id) => id === previous) ? previous : (ids[0] ?? "");
}

/**
 * Restore via cloud workspace id, then open the destination folder.
 * Does not create Personal or unlock a local DEK first.
 */
export async function runEmptyShellRestore(
  input: EmptyRestoreSelection,
  commands: Pick<EmptyRestoreCommands, "restore" | "openWorkspace">,
): Promise<EmptyRestoreOutcome> {
  const validated = validateEmptyRestoreSelection(input);
  if (!validated.ok) {
    return validated;
  }
  try {
    const result = await commands.restore(
      validated.cloudWorkspaceId,
      validated.targetRoot,
      validated.backupId,
    );
    await commands.openWorkspace(validated.targetRoot);
    return { ok: true, result, targetRoot: validated.targetRoot };
  } catch (err: unknown) {
    return { ok: false, error: userSafeEmptyRestoreError(err) };
  }
}
