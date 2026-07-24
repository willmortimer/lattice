import { invoke } from "@tauri-apps/api/core";

import type { DataAppSnapshot } from "./types";

/** Reload the package data-app snapshot after mutations (forms, records, etc.). */
export async function openPackageSnapshot(
  root: string,
  relPath: string,
): Promise<DataAppSnapshot> {
  return invoke<DataAppSnapshot>("open_data_app", {
    root,
    relPath,
    viewName: null,
    limit: null,
    offset: null,
  });
}
