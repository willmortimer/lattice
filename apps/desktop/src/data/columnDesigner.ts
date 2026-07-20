import type { FieldType } from "./types";
import { fieldTypeLabel } from "./recordDetail";

/** Field types exposed in the add-column designer (matches `lattice_data::FieldType`). */
export const COLUMN_FIELD_TYPES: FieldType[] = [
  "text",
  "long_text",
  "integer",
  "decimal",
  "boolean",
  "date",
  "relation",
];

export function columnFieldTypeOptions(): Array<{ value: FieldType; label: string }> {
  return COLUMN_FIELD_TYPES.map((fieldType) => ({
    value: fieldType,
    label: fieldTypeLabel(fieldType),
  }));
}

const COLUMN_NAME_PATTERN = /^[A-Za-z_][A-Za-z0-9_]*$/;

export function validateColumnName(name: string, existingNames: string[]): string | null {
  const trimmed = name.trim();
  if (!trimmed) {
    return "Column name is required.";
  }
  if (!COLUMN_NAME_PATTERN.test(trimmed)) {
    return "Use letters, numbers, and underscores; start with a letter or underscore.";
  }
  if (existingNames.some((existing) => existing.toLowerCase() === trimmed.toLowerCase())) {
    return `Column "${trimmed}" already exists.`;
  }
  return null;
}

export function validateRelationTarget(
  fieldType: FieldType,
  relationTable: string | undefined,
  availableTables: string[],
  currentTable: string,
): string | null {
  if (fieldType !== "relation") {
    return null;
  }
  const target = relationTable?.trim();
  if (!target) {
    return "Choose a target table for relation columns.";
  }
  if (target === currentTable) {
    return "Relation columns cannot target the same table.";
  }
  if (!availableTables.includes(target)) {
    return `Table "${target}" was not found in this package.`;
  }
  return null;
}

export interface AddColumnPayload {
  name: string;
  field_type: FieldType;
  relation_table?: string;
}

export function buildAddColumnPayload(
  name: string,
  fieldType: FieldType,
  relationTable: string | undefined,
): AddColumnPayload {
  const trimmed = name.trim();
  if (fieldType === "relation") {
    return {
      name: trimmed,
      field_type: fieldType,
      relation_table: relationTable?.trim(),
    };
  }
  return {
    name: trimmed,
    field_type: fieldType,
  };
}
