import type { FieldType } from "./types";

/** Field types users may assign during CSV import (relation excluded). */
export const CSV_IMPORT_FIELD_TYPES: ReadonlyArray<{ value: FieldType; label: string }> = [
  { value: "text", label: "Text" },
  { value: "long_text", label: "Long text" },
  { value: "integer", label: "Integer" },
  { value: "decimal", label: "Decimal" },
  { value: "boolean", label: "Boolean" },
  { value: "date", label: "Date" },
];

export interface CsvColumnPreview {
  name: string;
  field_type: string;
  sample_values: string[];
}

export interface CsvImportPreview {
  columns: CsvColumnPreview[];
  row_count: number;
  sample_rows: string[][];
}

export interface CsvColumnChoice {
  name: string;
  field_type: FieldType;
}

export interface CsvImportReviewState {
  csvPath: string;
  packageName: string;
  title: string;
  tableName: string;
  preview: CsvImportPreview;
  columns: CsvColumnChoice[];
}

const CSV_IMPORT_FIELD_TYPE_SET = new Set<FieldType>(
  CSV_IMPORT_FIELD_TYPES.map((entry) => entry.value),
);

export function isCsvImportFieldType(value: string): value is FieldType {
  return CSV_IMPORT_FIELD_TYPE_SET.has(value as FieldType);
}

export function normalizeCsvImportFieldType(value: string): FieldType {
  return isCsvImportFieldType(value) ? value : "text";
}

export function columnChoicesFromPreview(preview: CsvImportPreview): CsvColumnChoice[] {
  return preview.columns.map((column) => ({
    name: column.name,
    field_type: normalizeCsvImportFieldType(column.field_type),
  }));
}

export function fieldTypeLabel(fieldType: FieldType): string {
  const match = CSV_IMPORT_FIELD_TYPES.find((entry) => entry.value === fieldType);
  return match?.label ?? fieldType;
}

export function workspaceCsvAbsolutePath(workspaceRoot: string, relPath: string): string {
  const root = workspaceRoot.replace(/\/+$/g, "");
  const rel = relPath.replace(/^\/+/g, "");
  return rel ? `${root}/${rel}` : root;
}

export function defaultPackageNameFromCsvPath(relPath: string): string {
  const slash = relPath.lastIndexOf("/");
  const filename = slash >= 0 ? relPath.slice(slash + 1) : relPath;
  return filename.replace(/\.(csv|tsv)$/i, "") || "Imported";
}

export function tableNameFromPackageLabel(label: string): string {
  let name = label.trim().replace(/\.data$/i, "").toLowerCase()
    .replace(/[^a-z0-9_]+/g, "_").replace(/^_+|_+$/g, "");
  if (!name || /^\d/.test(name)) name = `t_${name || "table"}`;
  return name;
}

export function buildCsvImportReviewState(
  csvPath: string,
  packageName: string,
  preview: CsvImportPreview,
): CsvImportReviewState {
  const trimmed = packageName.trim();
  return {
    csvPath,
    packageName: trimmed,
    title: trimmed.replace(/\.data$/i, ""),
    tableName: tableNameFromPackageLabel(trimmed),
    preview,
    columns: columnChoicesFromPreview(preview),
  };
}
