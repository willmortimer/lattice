import { describe, expect, it } from "vitest";

import { cellValueToDisplay } from "./types";

describe("cellValueToDisplay", () => {
  it("handles Rust unit Null serialized as the JSON string Null", () => {
    expect(cellValueToDisplay("Null")).toBe("");
  });

  it("handles object Null and missing values", () => {
    expect(cellValueToDisplay({ Null: null })).toBe("");
    expect(cellValueToDisplay(undefined)).toBe("");
    expect(cellValueToDisplay(null)).toBe("");
  });

  it("formats relation record_ids without throwing on malformed payloads", () => {
    expect(cellValueToDisplay({ Relation: { record_ids: ["a", "b"] } })).toBe("a, b");
    expect(cellValueToDisplay({ Relation: { record_ids: [] } })).toBe("");
    expect(cellValueToDisplay({ Relation: {} as { record_ids: string[] } })).toBe("");
  });

  it("formats ordinary typed cells", () => {
    expect(cellValueToDisplay({ Text: "Ada" })).toBe("Ada");
    expect(cellValueToDisplay({ Integer: 7 })).toBe("7");
    expect(cellValueToDisplay({ Boolean: true })).toBe("true");
  });

  it("formats lookup display values", () => {
    expect(cellValueToDisplay({ Lookup: { values: ["Acme", "Beta"] } })).toBe("Acme, Beta");
    expect(cellValueToDisplay({ Lookup: { values: [] } })).toBe("");
  });

  it("formats rollup display values", () => {
    expect(cellValueToDisplay({ Rollup: { value: 2 } })).toBe("2");
    expect(cellValueToDisplay({ Rollup: { value: 15.5 } })).toBe("15.5");
    expect(cellValueToDisplay({ Rollup: { value: null } })).toBe("");
  });
});
