import { invoke } from "./ipc";
import { getCloudSessionStatus } from "./cloud";

export interface WorkspaceCryptoStatus {
  unlocked: boolean;
  workspaceId?: string;
}

export interface EncryptedBackupPutResult {
  backupId: string;
  cloudWorkspaceId: string;
  contentHash: string;
  ciphertextBytes: number;
  plaintextBytes: number;
}

export interface EncryptedBackupSkippedEntry {
  path: string;
  reason: string;
}

export interface EncryptedBackupRestoreResult {
  backupId: string;
  restoredCount: number;
  skipped: EncryptedBackupSkippedEntry[];
}

export interface EncryptedBackupListEntry {
  id: string;
  workspaceId: string;
  deviceId?: string | null;
  size: number;
  contentHash: string;
  createdAt: number;
}

const SIGN_IN_PREFIX = "Sign in under Settings → Cloud account";

function backupCreatedAtMs(createdAt: number): number {
  return createdAt > 1_000_000_000_000 ? createdAt : createdAt * 1000;
}

function formatBackupSize(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "0 B";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** Picker label: created time, size, and content hash (Inspect + Settings). */
export function formatEncryptedBackupOption(backup: EncryptedBackupListEntry): string {
  const created = new Date(backupCreatedAtMs(backup.createdAt)).toLocaleString();
  const hash =
    backup.contentHash.length > 12 ? backup.contentHash.slice(0, 12) : backup.contentHash;
  return `${created} · ${formatBackupSize(backup.size)} · ${hash}`;
}

export async function getWorkspaceCryptoStatus(): Promise<WorkspaceCryptoStatus> {
  return invoke<WorkspaceCryptoStatus>("workspace_crypto_status_cmd");
}

export async function unlockWorkspaceCrypto(
  workspaceId: string,
): Promise<WorkspaceCryptoStatus> {
  return invoke<WorkspaceCryptoStatus>("workspace_crypto_unlock_cmd", { workspaceId });
}

export async function lockWorkspaceCrypto(): Promise<WorkspaceCryptoStatus> {
  return invoke<WorkspaceCryptoStatus>("workspace_crypto_lock_cmd");
}

/**
 * Unlock DEK in Rust, encrypt a workspace backup payload, and PUT opaque bytes to
 * lattice-server `PUT /v1/workspaces/{id}/backups`. Requires cloud sign-in.
 */
export async function putEncryptedWorkspaceBackup(
  root: string,
  workspaceId: string,
): Promise<EncryptedBackupPutResult> {
  const session = await getCloudSessionStatus();
  if (!session.signedIn) {
    throw new Error(
      `${SIGN_IN_PREFIX} before uploading an encrypted backup.`,
    );
  }
  await unlockWorkspaceCrypto(workspaceId);
  return invoke<EncryptedBackupPutResult>("put_encrypted_workspace_backup_cmd", { root });
}

/**
 * List encrypted workspace backups for the open workspace. HTTP-only — does not
 * unlock the DEK.
 */
export async function listEncryptedWorkspaceBackups(
  root: string,
  workspaceId: string,
): Promise<EncryptedBackupListEntry[]> {
  const session = await getCloudSessionStatus();
  if (!session.signedIn) {
    throw new Error(
      `${SIGN_IN_PREFIX} before listing encrypted backups.`,
    );
  }
  if (!workspaceId.trim()) {
    throw new Error("Open a workspace before listing encrypted backups.");
  }
  return invoke<EncryptedBackupListEntry[]>("list_encrypted_workspace_backups_cmd", { root });
}

/**
 * Unlock DEK in Rust, GET opaque ciphertext, decrypt, and restore into `targetRoot`
 * (conflict-safe). When `backupId` is omitted, the latest cloud backup is used.
 */
export async function restoreEncryptedWorkspaceBackup(
  root: string,
  targetRoot: string,
  workspaceId: string,
  backupId?: string,
): Promise<EncryptedBackupRestoreResult> {
  const session = await getCloudSessionStatus();
  if (!session.signedIn) {
    throw new Error(
      `${SIGN_IN_PREFIX} before restoring an encrypted backup.`,
    );
  }
  await unlockWorkspaceCrypto(workspaceId);
  return invoke<EncryptedBackupRestoreResult>("restore_encrypted_workspace_backup_cmd", {
    root,
    targetRoot,
    backupId,
  });
}
