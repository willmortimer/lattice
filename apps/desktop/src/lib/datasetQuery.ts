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
  const result = await queryDatasetArrow(root, relPath, request);
  const dump = dumpArrowTransport(result);
  return { result, dump, summary: formatArrowDumpSummary(dump) };
}
