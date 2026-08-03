import { inBrowser } from "../demo";
import { openSettingsDeepLink } from "../settings/settingsDeepLink";
import { cloudBlobMaterialize, cloudBlobOpen, getCloudSessionStatus } from "./cloud";
import { invoke } from "./ipc";
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

export type CloudReopenOutcome =
  | {
      ok: true;
      byteLength: number;
      hydrated: true;
      content: string;
      revision: string | null;
    }
  | {
      ok: true;
      byteLength: number;
      hydrated: false;
      reason: string;
    }
  | { ok: false; message: string };

/** MVP single-binding conflict: cloud already stores a different hash for this resource. */
export const CLOUD_BLOB_CONFLICT_MESSAGE =
  "Local content changed since this resource was bound in cloud. MVP keeps a single hash per resource and cannot overwrite that binding. Check Inspect → Properties for authority and content hash.";

const NON_UTF8_REOPEN_REASON =
  "Cloud bytes are not UTF-8 text, so the workspace file was not updated. Semantic reopen hydrates pages and other text resources.";

/** True when an IPC / cloud error is an MVP blob binding conflict (HTTP 409 or duplicate put). */
export function isCloudBlobConflictError(message: string): boolean {
  const lower = message.toLowerCase();
  if (lower.includes("cloud api error (409)")) return true;
  if (lower.includes("blob already exists")) return true;
  if (/\bhttp 409\b/.test(lower)) return true;
  return lower.includes("(409)") && (lower.includes("bound") || lower.includes("conflict") || lower.includes("already"));
}

/** User-facing message for cloud blob / backup IPC failures. */
export function cloudBackupErrorMessage(err: unknown): string {
  const message = err instanceof Error ? err.message : String(err);
  if (message.includes("not signed in")) {
    return "Sign in under Settings → Cloud account to back up resources.";
  }
  if (isCloudBlobConflictError(message)) {
    return CLOUD_BLOB_CONFLICT_MESSAGE;
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

function decodeCloudText(bytes: number[]): string | null {
  try {
    return new TextDecoder("utf-8", { fatal: true }).decode(Uint8Array.from(bytes));
  } catch {
    return null;
  }
}

/**
 * Fetch canonical cloud bytes, then hydrate the workspace file through
 * `apply_page_update` (semantic save) when the payload is UTF-8 text.
 *
 * Binary / non-UTF-8 payloads return `hydrated: false` with an actionable reason;
 * callers still get the byte count from the cloud GET.
 */
export async function reopenResourceFromCloud(
  root: string,
  relPath: string,
): Promise<CloudReopenOutcome> {
  if (inBrowser) {
    return {
      ok: false,
      message: "Cloud reopen is not available in the browser demo.",
    };
  }

  const trimmed = relPath.trim();
  if (!trimmed) {
    return { ok: false, message: "Choose a resource to reopen from cloud." };
  }

  try {
    const bytes = await cloudBlobOpen(root, trimmed);
    const content = decodeCloudText(bytes);
    if (content === null) {
      return {
        ok: true,
        byteLength: bytes.length,
        hydrated: false,
        reason: NON_UTF8_REOPEN_REASON,
      };
    }

    // base_revision is local disk metadata; content from read_page may already be cloud.
    // Always write cloud bytes so a stale local file is brought in sync.
    const page = await invoke<{ content: string; revision: string }>("read_page", {
      root,
      relPath: trimmed,
    });
    const revision = await invoke<string>("apply_page_update", {
      root,
      relPath: trimmed,
      content,
      baseRevision: page.revision,
    });
    return {
      ok: true,
      byteLength: bytes.length,
      hydrated: true,
      content,
      revision,
    };
  } catch (err: unknown) {
    return { ok: false, message: cloudBackupErrorMessage(err) };
  }
}
