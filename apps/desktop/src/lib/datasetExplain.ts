import { invoke } from "@tauri-apps/api/core";

export interface ExplainDatasetRequest {
  sql?: string;
}

export interface ExplainDatasetResponse {
  sql: string;
  plan: string;
}

export async function explainDataset(
  root: string,
  relPath: string,
  request: ExplainDatasetRequest = {},
): Promise<ExplainDatasetResponse> {
  return invoke<ExplainDatasetResponse>("explain_dataset", {
    root,
    relPath,
    request,
  });
}
