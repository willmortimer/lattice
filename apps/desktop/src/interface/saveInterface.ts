import { invoke } from "@tauri-apps/api/core";

import type { InterfaceDef } from "../lib/bindingSpec";

export async function savePackageInterface(options: {
  root: string;
  relPath: string;
  def: InterfaceDef;
}): Promise<InterfaceDef> {
  return invoke<InterfaceDef>("save_data_interface", {
    root: options.root,
    relPath: options.relPath,
    request: options.def,
  });
}

export async function queryDataSqlScalar(options: {
  root: string;
  relPath: string;
  sql: string;
  limit?: number;
}): Promise<{ value: string | number | null; column?: string }> {
  const result = await invoke<{ value: string | number | null; column?: string }>(
    "query_data_sql_scalar",
    {
      root: options.root,
      relPath: options.relPath,
      sql: options.sql,
      limit: options.limit ?? 1,
    },
  );
  return result;
}
