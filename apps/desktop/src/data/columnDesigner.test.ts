import { describe, expect, it } from "vitest";

import {
  buildAddColumnPayload,
  validateColumnName,
  validateRelationTarget,
} from "./columnDesigner";

describe("validateColumnName", () => {
  it("rejects empty and invalid identifiers", () => {
    expect(validateColumnName("", ["id"])).toMatch(/required/i);
    expect(validateColumnName("1bad", ["id"])).toMatch(/letters/i);
  });

  it("rejects duplicate names case-insensitively", () => {
    expect(validateColumnName("Name", ["id", "name"])).toMatch(/already exists/i);
  });

  it("accepts valid new names", () => {
    expect(validateColumnName("company_name", ["id"])).toBeNull();
  });
});

describe("validateRelationTarget", () => {
  const tables = ["contacts", "companies"];

  it("requires a target for relation fields", () => {
    expect(validateRelationTarget("relation", "", tables, "contacts")).toMatch(/target table/i);
  });

  it("rejects self-relations and unknown tables", () => {
    expect(validateRelationTarget("relation", "contacts", tables, "contacts")).toMatch(
      /same table/i,
    );
    expect(validateRelationTarget("relation", "missing", tables, "contacts")).toMatch(/not found/i);
  });

  it("accepts valid relation targets", () => {
    expect(validateRelationTarget("relation", "companies", tables, "contacts")).toBeNull();
    expect(validateRelationTarget("text", undefined, tables, "contacts")).toBeNull();
  });
});

describe("buildAddColumnPayload", () => {
  it("omits relation_table for non-relation fields", () => {
    expect(buildAddColumnPayload("name", "text", undefined)).toEqual({
      name: "name",
      field_type: "text",
    });
  });

  it("includes relation_table for relation fields", () => {
    expect(buildAddColumnPayload("company", "relation", "companies")).toEqual({
      name: "company",
      field_type: "relation",
      relation_table: "companies",
    });
  });

  it("includes lookup metadata for lookup fields", () => {
    expect(buildAddColumnPayload("company_name", "lookup", undefined, "company", "name")).toEqual({
      name: "company_name",
      field_type: "lookup",
      lookup_relation: "company",
      lookup_field: "name",
    });
  });

  it("includes rollup metadata for rollup fields", () => {
    expect(
      buildAddColumnPayload(
        "item_count",
        "rollup",
        undefined,
        undefined,
        undefined,
        "items",
        "count",
      ),
    ).toEqual({
      name: "item_count",
      field_type: "rollup",
      rollup_relation: "items",
      rollup_aggregate: "count",
    });
    expect(
      buildAddColumnPayload(
        "total_amount",
        "rollup",
        undefined,
        undefined,
        undefined,
        "items",
        "sum",
        "amount",
      ),
    ).toEqual({
      name: "total_amount",
      field_type: "rollup",
      rollup_relation: "items",
      rollup_aggregate: "sum",
      rollup_field: "amount",
    });
  });

  it("includes options for enum fields", () => {
    expect(buildAddColumnPayload("status", "enum", undefined, undefined, undefined, undefined, undefined, undefined, undefined, "Open, Closed")).toEqual({
      name: "status",
      field_type: "enum",
      options: ["Open", "Closed"],
    });
    expect(
      buildAddColumnPayload(
        "tags",
        "multi_enum",
        undefined,
        undefined,
        undefined,
        undefined,
        undefined,
        undefined,
        undefined,
        "a\nb",
      ),
    ).toEqual({
      name: "tags",
      field_type: "multi_enum",
      options: ["a", "b"],
    });
  });
});
