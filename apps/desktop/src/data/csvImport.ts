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
