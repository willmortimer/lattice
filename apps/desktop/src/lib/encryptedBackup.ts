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
      "Sign in under Settings → Cloud account before uploading an encrypted backup.",
    );
  }
  await unlockWorkspaceCrypto(workspaceId);
  return invoke<EncryptedBackupPutResult>("put_encrypted_workspace_backup_cmd", { root });
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
      "Sign in under Settings → Cloud account before restoring an encrypted backup.",
    );
  }
  await unlockWorkspaceCrypto(workspaceId);
  return invoke<EncryptedBackupRestoreResult>("restore_encrypted_workspace_backup_cmd", {
    root,
    targetRoot,
    backupId,
  });
}
