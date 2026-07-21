import { invoke } from "@tauri-apps/api/core";
import {
  dumpArrowTransport,
  formatArrowDumpSummary,
  type ArrowQueryResult,
  type ArrowTransportDump,
} from "./arrowIpc";
import {
  DatasetRequestAbortedError,
  guardedDatasetRequest,
  newDatasetSessionId,
} from "./datasetCancel";

export interface QueryDatasetArrowRequest {
  sql?: string;
  maxRows?: number;
  maxBytes?: number;
  /** Optional cancel session id; generated when an AbortSignal is provided. */
  sessionId?: string;
}

export async function queryDatasetArrow(
  root: string,
  relPath: string,
  request: QueryDatasetArrowRequest = {},
  signal?: AbortSignal,
): Promise<ArrowQueryResult> {
  const sessionId = signal
    ? (request.sessionId ?? newDatasetSessionId())
    : request.sessionId;

  const result = await guardedDatasetRequest(
    () =>
      invoke<ArrowQueryResult>("query_dataset_arrow", {
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

  if (result.cancelled) {
    throw new DatasetRequestAbortedError();
  }
  return result;
}

export async function loadDatasetArrowDump(
  root: string,
  relPath: string,
  request: QueryDatasetArrowRequest = {},
  signal?: AbortSignal,
): Promise<{ result: ArrowQueryResult; dump: ArrowTransportDump; summary: string }> {
  const sessionId = signal
    ? (request.sessionId ?? newDatasetSessionId())
    : request.sessionId;
  const withSession = sessionId ? { ...request, sessionId } : request;

  const primary = await queryDatasetArrow(root, relPath, withSession, signal);
  const primaryDump = dumpArrowTransport(primary);
  if (primaryDump.rowCount > 0 || request.sql) {
    return { result: primary, dump: primaryDump, summary: formatArrowDumpSummary(primaryDump) };
  }

  const normalizedPath = relPath.replace(/\\/g, "/");
  const fallbackSql = `SELECT * FROM read_csv_auto('${normalizedPath}/facts/**/*.csv', union_by_name = true)`;
  const fallback = await queryDatasetArrow(
    root,
    relPath,
    { ...withSession, sql: fallbackSql },
    signal,
  );
  const fallbackDump = dumpArrowTransport(fallback);
  return { result: fallback, dump: fallbackDump, summary: formatArrowDumpSummary(fallbackDump) };
}
