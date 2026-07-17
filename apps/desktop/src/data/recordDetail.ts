import type { CellValue, DataColumn, DataRow, FieldType } from "./types";
import { cellValueToDisplay, displayToCellValue } from "./types";

export type RecordFieldEditorKind = "text" | "textarea" | "number" | "boolean" | "date";

export function fieldEditorKind(fieldType: FieldType): RecordFieldEditorKind {
  switch (fieldType) {
    case "long_text":
      return "textarea";
    case "integer":
    case "decimal":
      return "number";
    case "boolean":
      return "boolean";
    case "date":
      return "date";
    case "text":
    default:
      return "text";
  }
}

export function fieldTypeLabel(fieldType: FieldType): string {
  switch (fieldType) {
    case "long_text":
      return "Long text";
    case "integer":
      return "Integer";
    case "decimal":
      return "Decimal";
    case "boolean":
      return "Boolean";
    case "date":
      return "Date";
    case "text":
    default:
      return "Text";
  }
}

export function draftValuesFromRow(row: DataRow, columns: DataColumn[]): Record<string, string> {
  const draft: Record<string, string> = {};
  for (const column of columns) {
    draft[column.name] = cellValueToDisplay(row.values[column.name]);
  }
  return draft;
}

export function parseDraftField(text: string, fieldType: FieldType): CellValue {
  return displayToCellValue(text, fieldType);
}

export function collectDirtyValues(
  draft: Record<string, string>,
  row: DataRow,
  columns: DataColumn[],
): Record<string, CellValue> {
  const changes: Record<string, CellValue> = {};
  for (const column of columns) {
    if (column.name === "id") continue;
    const next = parseDraftField(draft[column.name] ?? "", column.field_type);
    const current = row.values[column.name];
    if (cellValueToDisplay(current) !== cellValueToDisplay(next)) {
      changes[column.name] = next;
    }
  }
  return changes;
}

export function hasDraftChanges(
  draft: Record<string, string>,
  row: DataRow,
  columns: DataColumn[],
): boolean {
  return Object.keys(collectDirtyValues(draft, row, columns)).length > 0;
}

export function validateDraftField(text: string, fieldType: FieldType): string | null {
  const trimmed = text.trim();
  if (!trimmed) return null;
  switch (fieldType) {
    case "integer": {
      const parsed = Number.parseInt(trimmed, 10);
      if (Number.isNaN(parsed) || String(parsed) !== trimmed) {
        return "Enter a whole number";
      }
      return null;
    }
    case "decimal": {
      const parsed = Number.parseFloat(trimmed);
      if (Number.isNaN(parsed)) {
        return "Enter a valid number";
      }
      return null;
    }
    case "date": {
      if (!/^\d{4}-\d{2}-\d{2}$/.test(trimmed)) {
        return "Use YYYY-MM-DD";
      }
      return null;
    }
    case "text":
    case "long_text":
    case "boolean":
    default:
      return null;
  }
}

export function draftFieldErrors(
  draft: Record<string, string>,
  columns: DataColumn[],
): Record<string, string> {
  const errors: Record<string, string> = {};
  for (const column of columns) {
    if (column.name === "id") continue;
    const message = validateDraftField(draft[column.name] ?? "", column.field_type);
    if (message) {
      errors[column.name] = message;
    }
  }
  return errors;
}
