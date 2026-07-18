import { describe, expect, it } from "vitest";

import type { DataColumn } from "./types";
import {
  DEMO_PACKAGE_FORMS,
  collectPackageFormValues,
  emptyPackageFormDraft,
  formDisplayTitle,
  loadPackageForm,
  listPackageForms,
  missingFormFields,
  resolvePackageFormColumns,
} from "./forms";

const columns: DataColumn[] = [
  { name: "id", field_type: "text", sqlite_type: "TEXT" },
  { name: "name", field_type: "text", sqlite_type: "TEXT" },
  { name: "email", field_type: "text", sqlite_type: "TEXT" },
  { name: "status", field_type: "text", sqlite_type: "TEXT" },
  {
    name: "company",
    field_type: "relation",
    sqlite_type: "TEXT",
    relation_table: "companies",
  },
  { name: "notes", field_type: "long_text", sqlite_type: "TEXT" },
];

describe("package form helpers", () => {
  it("resolves form fields in declared order and skips missing", () => {
    expect(
      resolvePackageFormColumns(columns, ["status", "name", "missing", "id"]).map(
        (column) => column.name,
      ),
    ).toEqual(["status", "name"]);
    expect(missingFormFields(columns, ["name", "ghost", "id"])).toEqual(["ghost"]);
  });

  it("builds empty drafts and insert payloads from form columns", () => {
    const formColumns = resolvePackageFormColumns(columns, DEMO_PACKAGE_FORMS[0]!.fields);
    expect(emptyPackageFormDraft(formColumns)).toEqual({
      name: "",
      email: "",
      status: "",
      company: "[]",
    });
    expect(
      collectPackageFormValues(
        {
          name: "Grace",
          email: "grace@example.com",
          status: "Active",
          company: '["co_1"]',
        },
        formColumns,
      ),
    ).toEqual({
      name: { Text: "Grace" },
      email: { Text: "grace@example.com" },
      status: { Text: "Active" },
      company: { Relation: { record_ids: ["co_1"] } },
    });
  });

  it("prefers title for display when present", () => {
    expect(formDisplayTitle(DEMO_PACKAGE_FORMS[0]!)).toBe("Contact intake");
    expect(formDisplayTitle({ name: "bare", table: "contacts", fields: [] })).toBe("bare");
  });

  it("lists and loads demo fixture forms without Tauri", async () => {
    const names = await listPackageForms({
      root: "",
      relPath: "CRM.data",
      demo: true,
    });
    expect(names).toEqual(["ContactIntake"]);

    const form = await loadPackageForm({
      root: "",
      relPath: "CRM.data",
      name: "ContactIntake",
      demo: true,
    });
    expect(form.table).toBe("contacts");
    expect(form.fields).toContain("email");

    await expect(
      loadPackageForm({
        root: "",
        relPath: "CRM.data",
        name: "Missing",
        demo: true,
      }),
    ).rejects.toThrow(/Unknown demo form/);
  });
});
