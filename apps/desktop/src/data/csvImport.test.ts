import { describe, expect, it } from "vitest";
import {
  buildCsvImportReviewState,
  columnChoicesFromPreview,
  defaultPackageNameFromCsvPath,
  isCsvImportFieldType,
  normalizeCsvImportFieldType,
  tableNameFromPackageLabel,
  workspaceCsvAbsolutePath,
} from "./csvImport";

describe("csvImport", () => {
  it("recognizes supported import field types", () => {
    expect(isCsvImportFieldType("integer")).toBe(true);
    expect(isCsvImportFieldType("relation")).toBe(false);
    expect(isCsvImportFieldType("unknown")).toBe(false);
  });

  it("normalizes unknown inferred types to text", () => {
    expect(normalizeCsvImportFieldType("decimal")).toBe("decimal");
    expect(normalizeCsvImportFieldType("relation")).toBe("text");
  });

  it("joins workspace root and relative CSV paths", () => {
    expect(workspaceCsvAbsolutePath("/workspace", "Data/sample.csv")).toBe("/workspace/Data/sample.csv");
    expect(workspaceCsvAbsolutePath("/workspace/", "/Data/sample.csv")).toBe("/workspace/Data/sample.csv");
  });

  it("derives a default package name from a CSV path", () => {
    expect(defaultPackageNameFromCsvPath("Data/sample.csv")).toBe("sample");
    expect(defaultPackageNameFromCsvPath("metrics.tsv")).toBe("metrics");
  });

  it("normalizes table names from package labels", () => {
    expect(tableNameFromPackageLabel("Sales Q1")).toBe("sales_q1");
    expect(tableNameFromPackageLabel("2026")).toBe("t_2026");
  });

  it("builds review state from preview metadata", () => {
    const preview = {
      columns: [{ name: "name", field_type: "text", sample_values: ["Ada"] }],
      row_count: 1,
      sample_rows: [["Ada"]],
    };
    expect(buildCsvImportReviewState("/tmp/sample.csv", "Contacts", preview)).toEqual({
      csvPath: "/tmp/sample.csv",
      packageName: "Contacts",
      title: "Contacts",
      tableName: "contacts",
      preview,
      columns: [{ name: "name", field_type: "text" }],
    });
  });

  it("maps preview columns to editable choices", () => {
    const choices = columnChoicesFromPreview({
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
});
