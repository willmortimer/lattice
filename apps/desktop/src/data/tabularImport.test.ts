import { describe, expect, it } from "vitest";
import {
  buildTabularImportReviewState,
  columnChoicesFromPreview,
  defaultPackageNameFromImportPath,
  isTabularImportFieldType,
  normalizeTabularImportFieldType,
  tabularFormatFromPath,
  tabularImportReviewTitle,
  tableNameFromPackageLabel,
  workspaceTabularAbsolutePath,
} from "./tabularImport";

describe("tabularImport", () => {
  it("recognizes supported import field types", () => {
    expect(isTabularImportFieldType("integer")).toBe(true);
    expect(isTabularImportFieldType("relation")).toBe(false);
    expect(isTabularImportFieldType("unknown")).toBe(false);
  });

  it("normalizes unknown inferred types to text", () => {
    expect(normalizeTabularImportFieldType("decimal")).toBe("decimal");
    expect(normalizeTabularImportFieldType("relation")).toBe("text");
  });

  it("joins workspace root and relative import paths", () => {
    expect(workspaceTabularAbsolutePath("/workspace", "Data/sample.csv")).toBe("/workspace/Data/sample.csv");
    expect(workspaceTabularAbsolutePath("/workspace/", "/Data/sample.json")).toBe("/workspace/Data/sample.json");
  });

  it("derives a default package name from import paths", () => {
    expect(defaultPackageNameFromImportPath("Data/sample.csv")).toBe("sample");
    expect(defaultPackageNameFromImportPath("metrics.xlsx")).toBe("metrics");
    expect(defaultPackageNameFromImportPath("records.jsonl")).toBe("records");
  });

  it("detects tabular formats from file extensions", () => {
    expect(tabularFormatFromPath("/tmp/data.xlsx")).toBe("Excel");
    expect(tabularFormatFromPath("/tmp/data.json")).toBe("JSON");
    expect(tabularFormatFromPath("/tmp/data.ndjson")).toBe("JSONL");
    expect(tabularFormatFromPath("/tmp/data.csv")).toBe("CSV");
  });

  it("normalizes table names from package labels", () => {
    expect(tableNameFromPackageLabel("Sales Q1")).toBe("sales_q1");
    expect(tableNameFromPackageLabel("2026")).toBe("t_2026");
  });

  it("builds review state from preview metadata", () => {
    const preview = {
      format: "JSON" as const,
      columns: [{ name: "name", field_type: "text", sample_values: ["Ada"] }],
      row_count: 1,
      sample_rows: [["Ada"]],
    };
    expect(buildTabularImportReviewState("/tmp/sample.json", "Contacts", preview)).toEqual({
      sourcePath: "/tmp/sample.json",
      format: "JSON",
      packageName: "Contacts",
      title: "Contacts",
      tableName: "contacts",
      preview,
      columns: [{ name: "name", field_type: "text" }],
    });
  });

  it("maps preview columns to editable choices", () => {
    const choices = columnChoicesFromPreview({
      format: "CSV",
      columns: [
        { name: "name", field_type: "text", sample_values: ["Ada"] },
        { name: "count", field_type: "integer", sample_values: ["1"] },
      ],
      row_count: 1,
      sample_rows: [["Ada", "1"]],
    });
    expect(choices).toEqual([
      { name: "name", field_type: "text" },
      { name: "count", field_type: "integer" },
    ]);
  });

  it("labels review dialogs by format", () => {
    expect(tabularImportReviewTitle("Excel")).toBe("Review Excel import");
    expect(tabularImportReviewTitle("JSONL")).toBe("Review JSONL import");
    expect(tabularImportReviewTitle("CSV")).toBe("Review CSV import");
  });
});
