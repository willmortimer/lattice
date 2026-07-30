import type { PackStatus } from "../lib/packs";

/** Human-readable pack lifecycle for Settings → Packs / Features. */
export function packStatusLabel(status: PackStatus): string {
  switch (status) {
    case "missing":
      return "Not downloaded";
    case "downloading":
      return "Downloading…";
    case "ready":
      return "Ready";
    case "failed":
      return "Failed";
    case "unavailable":
      return "Unavailable";
    default: {
      const _exhaustive: never = status;
      return _exhaustive;
    }
  }
}

/** Download button caption; prefers busy while an action is in flight. */
export function packDownloadButtonLabel(status: PackStatus, busy: boolean): string {
  if (busy || status === "downloading") return "Downloading…";
  if (status === "ready") return "Downloaded";
  if (status === "unavailable") return "Unavailable";
  if (status === "failed") return "Retry download";
  return "Download";
}

/** True when Download should be disabled (ready, unavailable, or already in flight). */
export function isPackDownloadDisabled(status: PackStatus, busy: boolean): boolean {
  return busy || status === "ready" || status === "downloading" || status === "unavailable";
}
