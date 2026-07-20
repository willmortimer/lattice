import type { FieldType } from "./types";

/** Field types users may assign during tabular import (relation excluded). */
export const TABULAR_IMPORT_FIELD_TYPES: ReadonlyArray<{ value: FieldType; label: string }> = [
  { value: "text", label: "Text" },
  { value: "long_text", label: "Long text" },
  { value: "integer", label: "Integer" },
  { value: "decimal", label: "Decimal" },
  { value: "boolean", label: "Boolean" },
  { value: "date", label: "Date" },
];

export type TabularImportFormat = "CSV" | "Excel" | "JSON" | "JSONL";

export interface TabularColumnPreview {
  name: string;
  field_type: string;
  sample_values: string[];
}

export interface TabularImportPreview {
  format: TabularImportFormat;
  columns: TabularColumnPreview[];
  row_count: number;
  sample_rows: string[][];
}

export interface TabularColumnChoice {
  name: string;
  field_type: FieldType;
}

export interface TabularImportReviewState {
  sourcePath: string;
  format: TabularImportFormat;
  packageName: string;
  title: string;
  tableName: string;
  preview: TabularImportPreview;
  columns: TabularColumnChoice[];
}

/** @deprecated Use tabular import types. */
export const CSV_IMPORT_FIELD_TYPES = TABULAR_IMPORT_FIELD_TYPES;
/** @deprecated Use `TabularColumnPreview`. */
export type CsvColumnPreview = TabularColumnPreview;
/** @deprecated Use `TabularImportPreview`. */
export type CsvImportPreview = TabularImportPreview;
/** @deprecated Use `TabularColumnChoice`. */
export type CsvColumnChoice = TabularColumnChoice;
/** @deprecated Use `TabularImportReviewState`. */
export interface CsvImportReviewState extends TabularImportReviewState {
  csvPath: string;
}

const TABULAR_IMPORT_FIELD_TYPE_SET = new Set<FieldType>(
  TABULAR_IMPORT_FIELD_TYPES.map((entry) => entry.value),
);

const TABULAR_IMPORT_EXTENSIONS = ["csv", "tsv", "xlsx", "json", "jsonl", "ndjson"] as const;

export function isTabularImportFieldType(value: string): value is FieldType {
  return TABULAR_IMPORT_FIELD_TYPE_SET.has(value as FieldType);
}

/** @deprecated Use `isTabularImportFieldType`. */
export const isCsvImportFieldType = isTabularImportFieldType;

export function normalizeTabularImportFieldType(value: string): FieldType {
  return isTabularImportFieldType(value) ? value : "text";
}

/** @deprecated Use `normalizeTabularImportFieldType`. */
export const normalizeCsvImportFieldType = normalizeTabularImportFieldType;

export function columnChoicesFromPreview(preview: TabularImportPreview): TabularColumnChoice[] {
  return preview.columns.map((column) => ({
    name: column.name,
    field_type: normalizeTabularImportFieldType(column.field_type),
  }));
}

export function fieldTypeLabel(fieldType: FieldType): string {
  const match = TABULAR_IMPORT_FIELD_TYPES.find((entry) => entry.value === fieldType);
  return match?.label ?? fieldType;
}

export function workspaceTabularAbsolutePath(workspaceRoot: string, relPath: string): string {
  const root = workspaceRoot.replace(/\/+$/g, "");
  const rel = relPath.replace(/^\/+/g, "");
  return rel ? `${root}/${rel}` : root;
}

/** @deprecated Use `workspaceTabularAbsolutePath`. */
export const workspaceCsvAbsolutePath = workspaceTabularAbsolutePath;

export function tabularFormatFromPath(path: string): TabularImportFormat {
  const dot = path.lastIndexOf(".");
  const ext = dot >= 0 ? path.slice(dot + 1).toLowerCase() : "";
  switch (ext) {
    case "xlsx":
      return "Excel";
    case "json":
      return "JSON";
    case "jsonl":
    case "ndjson":
      return "JSONL";
    default:
      return "CSV";
  }
}

export function defaultPackageNameFromImportPath(relPath: string): string {
  const slash = relPath.lastIndexOf("/");
  const filename = slash >= 0 ? relPath.slice(slash + 1) : relPath;
  return filename.replace(/\.(csv|tsv|xlsx|json|jsonl|ndjson)$/i, "") || "Imported";
}

/** @deprecated Use `defaultPackageNameFromImportPath`. */
export const defaultPackageNameFromCsvPath = defaultPackageNameFromImportPath;

export function tableNameFromPackageLabel(label: string): string {
  let name = label.trim().replace(/\.data$/i, "").toLowerCase()
    .replace(/[^a-z0-9_]+/g, "_").replace(/^_+|_+$/g, "");
  if (!name || /^\d/.test(name)) name = `t_${name || "table"}`;
  return name;
}

export function buildTabularImportReviewState(
  sourcePath: string,
  packageName: string,
  preview: TabularImportPreview,
): TabularImportReviewState {
  const trimmed = packageName.trim();
  return {
    sourcePath,
    format: preview.format,
    packageName: trimmed,
    title: trimmed.replace(/\.data$/i, ""),
    tableName: tableNameFromPackageLabel(trimmed),
    preview,
    columns: columnChoicesFromPreview(preview),
  };
}

/** @deprecated Use `buildTabularImportReviewState`. */
export function buildCsvImportReviewState(
  csvPath: string,
  packageName: string,
  preview: TabularImportPreview,
): CsvImportReviewState {
  const state = buildTabularImportReviewState(csvPath, packageName, preview);
  return { ...state, csvPath: state.sourcePath };
}

export const TABULAR_IMPORT_FILE_FILTERS: { name: string; extensions: string[] }[] = [
  {
    name: "Tabular data",
    extensions: [...TABULAR_IMPORT_EXTENSIONS],
  },
];

export function tabularImportReviewTitle(format: TabularImportFormat): string {
  switch (format) {
    case "Excel":
      return "Review Excel import";
    case "JSON":
      return "Review JSON import";
    case "JSONL":
      return "Review JSONL import";
    default:
      return "Review CSV import";
  }
}
