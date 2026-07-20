import { invoke } from "@tauri-apps/api/core";
import {
  dumpArrowTransport,
  formatArrowDumpSummary,
  type ArrowQueryResult,
  type ArrowTransportDump,
} from "./arrowIpc";

export interface QueryDatasetArrowRequest {
  sql?: string;
  maxRows?: number;
  maxBytes?: number;
}

export async function queryDatasetArrow(
  root: string,
  relPath: string,
  request: QueryDatasetArrowRequest = {},
): Promise<ArrowQueryResult> {
  return invoke<ArrowQueryResult>("query_dataset_arrow", {
    root,
    relPath,
    request,
  });
}

export async function loadDatasetArrowDump(
  root: string,
  relPath: string,
  request: QueryDatasetArrowRequest = {},
): Promise<{ result: ArrowQueryResult; dump: ArrowTransportDump; summary: string }> {
  const primary = await queryDatasetArrow(root, relPath, request);
  const primaryDump = dumpArrowTransport(primary);
  if (primaryDump.rowCount > 0 || request.sql) {
    return { result: primary, dump: primaryDump, summary: formatArrowDumpSummary(primaryDump) };
  }

  const normalizedPath = relPath.replace(/\\/g, "/");
  const fallbackSql = `SELECT * FROM read_csv_auto('${normalizedPath}/facts/**/*.csv', union_by_name = true)`;
  const fallback = await queryDatasetArrow(root, relPath, { ...request, sql: fallbackSql });
  const fallbackDump = dumpArrowTransport(fallback);
  return { result: fallback, dump: fallbackDump, summary: formatArrowDumpSummary(fallbackDump) };
}
