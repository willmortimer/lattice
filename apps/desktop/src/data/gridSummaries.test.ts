import { describe, expect, it } from "vitest";
import {
  buildGroupedGridRows,
  computeLayoutSummary,
  summaryValuesForRows,
} from "./gridSummaries";
import type { DataColumn, DataRow } from "./types";

const columns: DataColumn[] = [
  { name: "status", field_type: "text", sqlite_type: "TEXT" },
  { name: "amount", field_type: "decimal", sqlite_type: "REAL" },
];

const rows: DataRow[] = [
  {
    id: "1",
    values: { status: { Text: "Active" }, amount: { Decimal: 10 } },
  },
  {
    id: "2",
    values: { status: { Text: "Active" }, amount: { Decimal: 5 } },
  },
  {
    id: "3",
    values: { status: { Text: "Done" }, amount: { Decimal: 3 } },
  },
];

describe("computeLayoutSummary", () => {
  it("computes count and numeric aggregates", () => {
    expect(computeLayoutSummary(rows, "status", "text", "count")).toBe(3);
    expect(computeLayoutSummary(rows, "amount", "decimal", "sum")).toBe(18);
    expect(computeLayoutSummary(rows.slice(0, 2), "amount", "decimal", "max")).toBe(10);
  });
});

describe("buildGroupedGridRows", () => {
  it("inserts group headers, footers, and a grand footer", () => {
    const summaries = [
      { field: "amount", aggregate: "sum" as const },
      { field: "status", aggregate: "count" as const },
    ];
    const gridRows = buildGroupedGridRows(rows, columns, "status", summaries);

    expect(gridRows.map((row) => row.kind)).toEqual([
      "group-header",
      "data",
      "data",
      "group-footer",
      "group-header",
      "data",
      "group-footer",
      "grand-footer",
    ]);
    expect(gridRows[3]?.summaryValues).toEqual({ amount: "15", status: "2" });
    expect(summaryValuesForRows(rows, columns, summaries)).toEqual({
      amount: "18",
      status: "3",
    });
  });

  it("supports footer-only summaries without grouping", () => {
    const gridRows = buildGroupedGridRows(rows, columns, undefined, [
      { field: "amount", aggregate: "sum" },
    ]);
    expect(gridRows.map((row) => row.kind)).toEqual(["data", "data", "data", "grand-footer"]);
  });
});
