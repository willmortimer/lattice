import { describe, expect, it } from "vitest";

import type { DataColumn, DataRow } from "./types";
import {
  collectDirtyValues,
  collectFormValues,
  draftFieldErrors,
  draftValuesFromRow,
  emptyDraftValues,
  fieldEditorKind,
  fieldTypeLabel,
  hasDraftChanges,
  parseDraftField,
  toggleRelationDraftId,
  validateDraftField,
} from "./recordDetail";
import { relationDraftFromIds } from "./relationDisplay";

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
    expect(fieldEditorKind("lookup")).toBe("lookup");
    expect(fieldEditorKind("rollup")).toBe("rollup");
    expect(fieldEditorKind("formula")).toBe("formula");
    expect(fieldEditorKind("enum")).toBe("enum");
    expect(fieldEditorKind("multi_enum")).toBe("multi_enum");
    expect(fieldTypeLabel("decimal")).toBe("Decimal");
    expect(fieldTypeLabel("lookup")).toBe("Lookup");
    expect(fieldTypeLabel("rollup")).toBe("Rollup");
    expect(fieldTypeLabel("formula")).toBe("Formula");
    expect(fieldTypeLabel("enum")).toBe("Enum");
    expect(fieldTypeLabel("multi_enum")).toBe("Multi enum");
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

  it("builds create-form drafts and insert payloads", () => {
    const editable = columns.filter((column) => column.name !== "id");
    expect(emptyDraftValues(editable)).toEqual({
      name: "",
      count: "",
      active: "false",
      notes: "",
      due: "",
    });
    expect(
      collectFormValues(
        { name: "Grace", count: "2", active: "true", notes: "", due: "2026-07-18" },
        editable,
      ),
    ).toEqual({
      name: { Text: "Grace" },
      count: { Integer: 2 },
      active: { Boolean: true },
      due: { Date: "2026-07-18" },
    });
  });

  it("round-trips relation drafts and detects relation changes", () => {
    const relationColumns: DataColumn[] = [
      { name: "id", field_type: "text", sqlite_type: "TEXT" },
      {
        name: "company",
        field_type: "relation",
        sqlite_type: "TEXT",
        relation_table: "companies",
      },
    ];
    const relationRow: DataRow = {
      id: "rec_1",
      values: {
        id: { Text: "rec_1" },
        company: { Relation: { record_ids: ["co_1"] } },
      },
    };

    expect(fieldEditorKind("relation")).toBe("relation");
    expect(fieldTypeLabel("relation")).toBe("Relation");
    expect(draftValuesFromRow(relationRow, relationColumns)).toEqual({
      id: "rec_1",
      company: relationDraftFromIds(["co_1"]),
    });
    expect(parseDraftField(relationDraftFromIds(["co_1", "co_2"]), "relation")).toEqual({
      Relation: { record_ids: ["co_1", "co_2"] },
    });
    expect(validateDraftField('["co_1"]', "relation")).toBeNull();
    expect(validateDraftField("bad", "relation")).toBe(
      "Relation value must be a JSON array of record ids",
    );

    const draft = draftValuesFromRow(relationRow, relationColumns);
    expect(hasDraftChanges(draft, relationRow, relationColumns)).toBe(false);
    const updatedDraft = {
      ...draft,
      company: toggleRelationDraftId(draft.company, "co_2", true),
    };
    expect(hasDraftChanges(updatedDraft, relationRow, relationColumns)).toBe(true);
    expect(collectDirtyValues(updatedDraft, relationRow, relationColumns)).toEqual({
      company: { Relation: { record_ids: ["co_1", "co_2"] } },
    });
  });
});
