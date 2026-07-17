import { cleanupHistory, type HistoryCleanupReport } from "../lib/revisions";

export const DEFAULT_HISTORY_RETENTION_DAYS = 180;
export const DEFAULT_HISTORY_RETENTION_BYTES = 1024 * 1024 * 1024;

export interface HistoryRetentionControls {
  maxAgeDays: number;
  maxBytes: number;
}

export function defaultHistoryRetentionControls(): HistoryRetentionControls {
  return {
    maxAgeDays: DEFAULT_HISTORY_RETENTION_DAYS,
    maxBytes: DEFAULT_HISTORY_RETENTION_BYTES,
  };
}

export function gibibytesToBytes(gib: number): number {
  return Math.round(gib * 1024 * 1024 * 1024);
}

export function bytesToGibibytes(bytes: number): number {
  return Math.round((bytes / (1024 * 1024 * 1024)) * 100) / 100;
}

export function formatReclaimableBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KiB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MiB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GiB`;
}

/** Preview first; only call destructive cleanup after requiresConfirmation is cleared. */
export async function previewHistoryCleanup(
  root: string,
  controls: HistoryRetentionControls,
): Promise<HistoryCleanupReport> {
  return cleanupHistory({
    root,
    dryRun: true,
    maxAgeDays: controls.maxAgeDays,
    maxBytes: controls.maxBytes,
  });
}

export async function confirmHistoryCleanup(
  root: string,
  controls: HistoryRetentionControls,
): Promise<HistoryCleanupReport> {
  return cleanupHistory({
    root,
    dryRun: false,
    maxAgeDays: controls.maxAgeDays,
    maxBytes: controls.maxBytes,
  });
}
