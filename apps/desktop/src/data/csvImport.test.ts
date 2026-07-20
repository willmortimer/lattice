import { describe, expect, it } from "vitest";
import {
  columnChoicesFromPreview,
  isCsvImportFieldType,
  normalizeCsvImportFieldType,
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
