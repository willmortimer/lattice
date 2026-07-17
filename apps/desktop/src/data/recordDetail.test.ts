import { describe, expect, it } from "vitest";

import type { DataColumn, DataRow } from "./types";
import {
  collectDirtyValues,
  draftFieldErrors,
  draftValuesFromRow,
  fieldEditorKind,
  fieldTypeLabel,
  hasDraftChanges,
  validateDraftField,
} from "./recordDetail";

const columns: DataColumn[] = [
  { name: "id", field_type: "text", sqlite_type: "TEXT" },
  { name: "name", field_type: "text", sqlite_type: "TEXT" },
  { name: "count", field_type: "integer", sqlite_type: "INTEGER" },
  { name: "active", field_type: "boolean", sqlite_type: "INTEGER" },
  { name: "notes", field_type: "long_text", sqlite_type: "TEXT" },
  { name: "due", field_type: "date", sqlite_type: "TEXT" },
];

const row: DataRow = {
  id: "rec_1",
  values: {
    id: { Text: "rec_1" },
    name: { Text: "Ada" },
    count: { Integer: 3 },
    active: { Boolean: true },
    notes: { Null: null },
    due: { Date: "2026-07-17" },
  },
};

describe("recordDetail helpers", () => {
  it("maps field types to editor kinds and labels", () => {
    expect(fieldEditorKind("long_text")).toBe("textarea");
    expect(fieldEditorKind("integer")).toBe("number");
    expect(fieldEditorKind("boolean")).toBe("boolean");
    expect(fieldTypeLabel("decimal")).toBe("Decimal");
  });

  it("builds draft strings from row values", () => {
    expect(draftValuesFromRow(row, columns)).toEqual({
      id: "rec_1",
      name: "Ada",
      count: "3",
      active: "true",
      notes: "",
      due: "2026-07-17",
    });
  });

  it("detects dirty fields and collects update payload", () => {
    const draft = { ...draftValuesFromRow(row, columns), name: "Grace", count: "5" };
    expect(hasDraftChanges(draft, row, columns)).toBe(true);
    expect(collectDirtyValues(draft, row, columns)).toEqual({
      name: { Text: "Grace" },
      count: { Integer: 5 },
    });
  });

  it("reports no changes when draft matches row", () => {
    const draft = draftValuesFromRow(row, columns);
    expect(hasDraftChanges(draft, row, columns)).toBe(false);
    expect(collectDirtyValues(draft, row, columns)).toEqual({});
  });

  it("validates numeric and date draft values", () => {
    expect(validateDraftField("12", "integer")).toBeNull();
    expect(validateDraftField("12.5", "integer")).toBe("Enter a whole number");
    expect(validateDraftField("abc", "decimal")).toBe("Enter a valid number");
    expect(validateDraftField("2026-07-17", "date")).toBeNull();
    expect(validateDraftField("07/17/2026", "date")).toBe("Use YYYY-MM-DD");
  });

  it("aggregates per-field validation errors", () => {
    const draft = { ...draftValuesFromRow(row, columns), count: "nope", due: "bad" };
    expect(draftFieldErrors(draft, columns)).toEqual({
      count: "Enter a whole number",
      due: "Use YYYY-MM-DD",
    });
  });
});
