import { inBrowser } from "../demo";
import { openSettingsDeepLink } from "../settings/settingsDeepLink";
import { cloudBlobMaterialize, getCloudSessionStatus } from "./cloud";
import type { ResourceStat } from "./resourceStat";

export type CloudBackupFailureReason = "signed_out" | "browser" | "error";

export interface CloudBackupFailure {
  ok: false;
  reason: CloudBackupFailureReason;
  message: string;
}

export type CloudBackupOutcome =
  | { ok: true; stat: ResourceStat }
  | CloudBackupFailure;

/** User-facing message for cloud blob / backup IPC failures. */
export function cloudBackupErrorMessage(err: unknown): string {
  const message = err instanceof Error ? err.message : String(err);
  if (message.includes("not signed in")) {
    return "Sign in under Settings → Cloud account to back up resources.";
  }
  return message;
}

/** Navigate to Settings → Cloud account (also switches activity to Settings). */
export function openCloudAccountSettings(): boolean {
  return openSettingsDeepLink("cloud");
}

/** Whether a workspace resource can be uploaded to Lattice Cloud. */
export function isCloudBackupResource(kind: string): boolean {
  return kind !== "folder";
}

/**
 * Gate on cloud session, materialize local bytes to cloud, and return updated stat.
 * Signed-out callers should use `openCloudAccountSettings()` when reason is signed_out.
 */
export async function backupResourceToCloud(
  root: string,
  relPath: string,
): Promise<CloudBackupOutcome> {
  if (inBrowser) {
    return {
      ok: false,
      reason: "browser",
      message: "Cloud backup is not available in the browser demo.",
    };
  }

  const trimmed = relPath.trim();
  if (!trimmed) {
    return {
      ok: false,
      reason: "error",
      message: "Choose a resource to back up.",
    };
  }

  const session = await getCloudSessionStatus();
  if (!session.signedIn) {
    return {
      ok: false,
      reason: "signed_out",
      message: "Sign in under Settings → Cloud account to back up resources.",
    };
  }

  try {
    const stat = await cloudBlobMaterialize(root, trimmed);
    return { ok: true, stat };
  } catch (err: unknown) {
    const message = cloudBackupErrorMessage(err);
    const reason: CloudBackupFailureReason = message.includes("Sign in under Settings")
      ? "signed_out"
      : "error";
    return { ok: false, reason, message };
  }
}
