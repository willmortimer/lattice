import { invoke } from "@tauri-apps/api/core";
import { guardedDatasetRequest } from "./datasetCancel";

export interface ExplainDatasetRequest {
  sql?: string;
}

export interface ExplainDatasetResponse {
  sql: string;
  plan: string;
}

/**
 * Run DuckDB EXPLAIN for a dataset package.
 *
 * AbortSignal cancels the frontend wait (panel switch / Cancel). Plan does not
 * register a backend cancel session — EXPLAIN is typically fast.
 */
export async function explainDataset(
  root: string,
  relPath: string,
  request: ExplainDatasetRequest = {},
  signal?: AbortSignal,
): Promise<ExplainDatasetResponse> {
  return guardedDatasetRequest(
    () =>
      invoke<ExplainDatasetResponse>("explain_dataset", {
        root,
        relPath,
        request,
      }),
    signal,
  );
}
