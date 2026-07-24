import { invoke } from "@tauri-apps/api/core";

import type { DataAppSnapshot } from "./types";

/** Default row window for interface-embedded saved views. */
export const EMBEDDED_DATA_VIEW_ROW_LIMIT = 50;

/** Load a bounded snapshot for a named saved view (filters, sort, layout metadata). */
export async function openEmbeddedDataView(
  root: string,
  relPath: string,
  viewName: string,
  limit = EMBEDDED_DATA_VIEW_ROW_LIMIT,
): Promise<DataAppSnapshot> {
  return invoke<DataAppSnapshot>("open_data_app", {
    root,
    relPath,
    viewName,
    limit,
    offset: 0,
  });
}
