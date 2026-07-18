import { invoke } from "@tauri-apps/api/core";

import type { CellValue, DataColumn } from "./types";
import { collectFormValues, emptyDraftValues } from "./recordDetail";

/** Mirrors Tauri `FormSummary` from `list_data_forms` / `load_data_form`. */
export interface FormSummary {
  name: string;
  table: string;
  fields: string[];
  title?: string;
  description?: string;
}

/**
 * Browser-demo fixture forms (no package seed until F3). Used when
 * `demoMutate` is active so the Forms chrome can be exercised locally.
 */
export const DEMO_PACKAGE_FORMS: FormSummary[] = [
  {
    name: "ContactIntake",
    table: "contacts",
    fields: ["name", "email", "status", "company"],
    title: "Contact intake",
    description: "Create a contact from the package form (demo fixture).",
  },
];

export function formDisplayTitle(form: FormSummary): string {
  return form.title?.trim() || form.name;
}

/** Resolve FormDef field names to table columns, preserving form order. */
export function resolvePackageFormColumns(
  columns: DataColumn[],
  fields: string[],
): DataColumn[] {
  const byName = new Map(columns.map((column) => [column.name, column]));
  const resolved: DataColumn[] = [];
  for (const field of fields) {
    if (field === "id") continue;
    const column = byName.get(field);
    if (column) {
      resolved.push(column);
    }
  }
  return resolved;
}

export function missingFormFields(columns: DataColumn[], fields: string[]): string[] {
  const names = new Set(columns.map((column) => column.name));
  return fields.filter((field) => field !== "id" && !names.has(field));
}

export function emptyPackageFormDraft(columns: DataColumn[]): Record<string, string> {
  return emptyDraftValues(columns);
}

export function collectPackageFormValues(
  draft: Record<string, string>,
  columns: DataColumn[],
): Record<string, CellValue> {
  return collectFormValues(draft, columns);
}

export async function listDataForms(root: string, relPath: string): Promise<string[]> {
  return invoke<string[]>("list_data_forms", { root, relPath });
}

export async function loadDataForm(
  root: string,
  relPath: string,
  name: string,
): Promise<FormSummary> {
  return invoke<FormSummary>("load_data_form", { root, relPath, name });
}

/** List forms for native IPC or return demo fixtures in browser mode. */
export async function listPackageForms(options: {
  root: string;
  relPath: string;
  demo?: boolean;
  demoForms?: FormSummary[];
}): Promise<string[]> {
  if (options.demo) {
    return (options.demoForms ?? DEMO_PACKAGE_FORMS).map((form) => form.name);
  }
  return listDataForms(options.root, options.relPath);
}

/** Load one form for native IPC or resolve from demo fixtures. */
export async function loadPackageForm(options: {
  root: string;
  relPath: string;
  name: string;
  demo?: boolean;
  demoForms?: FormSummary[];
}): Promise<FormSummary> {
  if (options.demo) {
    const forms = options.demoForms ?? DEMO_PACKAGE_FORMS;
    const match = forms.find((form) => form.name === options.name);
    if (!match) {
      throw new Error(`Unknown demo form: ${options.name}`);
    }
    return match;
  }
  return loadDataForm(options.root, options.relPath, options.name);
}
