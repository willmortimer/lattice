import { invoke } from "@tauri-apps/api/core";

import { demoPackageActions, demoPackageActionsByPath } from "../demoWorkspace.generated";
import type { CellValue, DataColumn } from "./types";
import { displayToCellValue, type FieldType } from "./types";

/** Mirrors Tauri `ActionSummary` from `list_data_actions` / `load_data_action`. */
export type ActionKind =
  | {
      type: "insert_record";
      form?: string;
      defaults?: Record<string, string>;
    }
  | {
      type: "update_field";
      field: string;
      value: string;
    }
  | {
      type: "open_url";
      url: string;
    };

export type ActionScope = "toolbar" | "row";

export interface ActionSummary {
  name: string;
  label: string;
  table: string;
  scope: ActionScope;
  action: ActionKind;
}

export const DEMO_PACKAGE_ACTIONS: ActionSummary[] = demoPackageActions;

export const DEMO_PACKAGE_ACTIONS_BY_PATH: Record<string, ActionSummary[]> =
  demoPackageActionsByPath ?? {};

export function demoActionsForPackage(relPath: string): ActionSummary[] {
  return DEMO_PACKAGE_ACTIONS_BY_PATH[relPath] ?? DEMO_PACKAGE_ACTIONS;
}

export function defaultsToCellValues(
  defaults: Record<string, string> | undefined,
  columns: DataColumn[],
): Record<string, CellValue> {
  if (!defaults) return {};
  const byName = new Map(columns.map((column) => [column.name, column]));
  const values: Record<string, CellValue> = {};
  for (const [field, raw] of Object.entries(defaults)) {
    const column = byName.get(field);
    if (!column) continue;
    values[field] = displayToCellValue(raw, column.field_type as FieldType);
  }
  return values;
}

export async function listDataActions(root: string, relPath: string): Promise<string[]> {
  return invoke<string[]>("list_data_actions", { root, relPath });
}

export async function loadDataAction(
  root: string,
  relPath: string,
  name: string,
): Promise<ActionSummary> {
  return invoke<ActionSummary>("load_data_action", { root, relPath, name });
}

export async function listPackageActions(options: {
  root: string;
  relPath: string;
  demo?: boolean;
  demoActions?: ActionSummary[];
}): Promise<ActionSummary[]> {
  if (options.demo) {
    return options.demoActions ?? demoActionsForPackage(options.relPath);
  }
  const names = await listDataActions(options.root, options.relPath);
  const loaded: ActionSummary[] = [];
  for (const name of names) {
    loaded.push(await loadDataAction(options.root, options.relPath, name));
  }
  return loaded;
}
