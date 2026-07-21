import { invoke } from "@tauri-apps/api/core";
import {
  DatasetRequestAbortedError,
  guardedDatasetRequest,
  isDatasetRequestAborted,
  newDatasetSessionId,
} from "./datasetCancel";

export interface ColumnProfile {
  name: string;
  dataType: string;
  rowCount?: number;
  nullPercentage?: number;
  approxDistinct?: number;
  min?: string;
  max?: string;
  avg?: number;
  std?: number;
  q25?: string;
  q50?: string;
  q75?: string;
}

export interface RelationProfile {
  rowCount: number;
  columns: ColumnProfile[];
  relationSql: string;
}

export interface ProfileDatasetRequest {
  sql?: string;
  /** Optional cancel session id; generated when an AbortSignal is provided. */
  sessionId?: string;
}

function isProfileCancelledMessage(error: unknown): boolean {
  if (!(error instanceof Error)) return false;
  const message = error.message.toLowerCase();
  return message.includes("profile cancelled") || message.includes("interrupted");
}

export async function profileDataset(
  root: string,
  relPath: string,
  request: ProfileDatasetRequest = {},
  signal?: AbortSignal,
): Promise<RelationProfile> {
  const sessionId = signal
    ? (request.sessionId ?? newDatasetSessionId())
    : request.sessionId;

  try {
    return await guardedDatasetRequest(
      () =>
        invoke<RelationProfile>("profile_dataset", {
          root,
          relPath,
          request: {
            ...request,
            ...(sessionId ? { sessionId } : {}),
          },
        }),
      signal,
      sessionId,
    );
  } catch (error: unknown) {
    if (isDatasetRequestAborted(error)) throw error;
    if (signal?.aborted || isProfileCancelledMessage(error)) {
      throw new DatasetRequestAbortedError();
    }
    throw error;
  }
}

export function formatPercent(value: number | undefined): string {
  if (value === undefined || Number.isNaN(value)) return "—";
  return `${value.toFixed(1)}%`;
}

export function formatDistinct(value: number | undefined): string {
  if (value === undefined) return "—";
  return `~${value.toLocaleString()}`;
}

export function formatProfileSummary(profile: RelationProfile): string {
  const columnCount = profile.columns.length;
  const rowLabel = profile.rowCount.toLocaleString();
  const columnLabel = columnCount === 1 ? "column" : "columns";
  return `${rowLabel} rows · ${columnCount} ${columnLabel}`;
}
