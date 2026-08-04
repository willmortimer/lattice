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
