import type { CellValue, DataColumn, DataRow, FieldType } from "./types";
import { cellValueToDisplay, displayToCellValue } from "./types";
import {
  extractRelationIds,
  parseRelationDraft,
  relationCellValue,
  relationDraftFromIds,
  relationIdsEqual,
} from "./relationDisplay";

export type RecordFieldEditorKind =
  | "text"
  | "textarea"
  | "number"
  | "boolean"
  | "date"
  | "relation"
  | "lookup";

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
    case "relation":
      return "relation";
    case "lookup":
      return "lookup";
    case "text":
      return "text";
    default: {
      const _exhaustive: never = fieldType;
      return _exhaustive;
    }
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
    case "relation":
      return "Relation";
    case "lookup":
      return "Lookup";
    case "text":
      return "Text";
    default: {
      const _exhaustive: never = fieldType;
      return _exhaustive;
    }
  }
}

export function draftValueFromCell(value: CellValue | undefined, fieldType: FieldType): string {
  if (fieldType === "relation") {
    return relationDraftFromIds(extractRelationIds(value));
  }
  return cellValueToDisplay(value);
}

export function draftValuesFromRow(row: DataRow, columns: DataColumn[]): Record<string, string> {
  const draft: Record<string, string> = {};
  for (const column of columns) {
    draft[column.name] = draftValueFromCell(row.values[column.name], column.field_type);
  }
  return draft;
}

export function parseDraftField(text: string, fieldType: FieldType): CellValue {
  if (fieldType === "relation") {
    return relationCellValue(parseRelationDraft(text));
  }
  return displayToCellValue(text, fieldType);
}

function draftFieldChanged(
  draftText: string,
  current: CellValue | undefined,
  fieldType: FieldType,
): boolean {
  if (fieldType === "relation") {
    return !relationIdsEqual(extractRelationIds(current), parseRelationDraft(draftText));
  }
  return cellValueToDisplay(current) !== cellValueToDisplay(parseDraftField(draftText, fieldType));
}

export function collectDirtyValues(
  draft: Record<string, string>,
  row: DataRow,
  columns: DataColumn[],
): Record<string, CellValue> {
  const changes: Record<string, CellValue> = {};
  for (const column of columns) {
    if (column.name === "id" || column.field_type === "lookup") continue;
    const draftText = draft[column.name] ?? "";
    if (!draftFieldChanged(draftText, row.values[column.name], column.field_type)) {
      continue;
    }
    changes[column.name] = parseDraftField(draftText, column.field_type);
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
    case "relation": {
      try {
        const parsed: unknown = JSON.parse(trimmed);
        if (!Array.isArray(parsed) || !parsed.every((entry) => typeof entry === "string")) {
          return "Relation value must be a JSON array of record ids";
        }
      } catch {
        return "Relation value must be a JSON array of record ids";
      }
      return null;
    }
    case "text":
    case "long_text":
    case "boolean":
    case "lookup":
      return null;
    default: {
      const _exhaustive: never = fieldType;
      return _exhaustive;
    }
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

export function emptyDraftValues(columns: DataColumn[]): Record<string, string> {
  const draft: Record<string, string> = {};
  for (const column of columns) {
    if (column.field_type === "boolean") {
      draft[column.name] = "false";
    } else if (column.field_type === "relation") {
      draft[column.name] = relationDraftFromIds([]);
    } else {
      draft[column.name] = "";
    }
  }
  return draft;
}

/** Build an insert payload from a create-form draft. */
export function collectFormValues(
  draft: Record<string, string>,
  columns: DataColumn[],
): Record<string, CellValue> {
  const values: Record<string, CellValue> = {};
  for (const column of columns) {
    if (column.name === "id" || column.field_type === "lookup") {
      continue;
    }
    const text = draft[column.name] ?? "";
    if (!text.trim() && column.field_type !== "boolean" && column.field_type !== "relation") {
      continue;
    }
    values[column.name] = parseDraftField(text, column.field_type);
  }
  return values;
}

export function toggleRelationDraftId(draftText: string, recordId: string, selected: boolean): string {
  const current = parseRelationDraft(draftText);
  const next = selected
    ? current.includes(recordId)
      ? current
      : [...current, recordId]
    : current.filter((id) => id !== recordId);
  return relationDraftFromIds(next);
}
