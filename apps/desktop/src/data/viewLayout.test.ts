import { describe, expect, it } from "vitest";

import type { DataColumn, DataRow } from "./types";
import {
  groupRowsByColumn,
  resolveGroupByColumn,
  resolveListPrimaryColumn,
  resolveListSubtitleColumn,
} from "./viewLayout";

const columns: DataColumn[] = [
  { name: "id", field_type: "text", sqlite_type: "TEXT" },
  { name: "name", field_type: "text", sqlite_type: "TEXT" },
  { name: "status", field_type: "text", sqlite_type: "TEXT" },
  { name: "active", field_type: "boolean", sqlite_type: "INTEGER" },
  { name: "count", field_type: "integer", sqlite_type: "INTEGER" },
];

const rows: DataRow[] = [
  {
    id: "a",
    values: {
      id: { Text: "a" },
      name: { Text: "Ada" },
      status: { Text: "Done" },
      active: { Boolean: true },
      count: { Integer: 1 },
    },
  },
  {
    id: "b",
    values: {
      id: { Text: "b" },
      name: { Text: "Grace" },
      status: { Text: "Active" },
      active: { Boolean: false },
      count: { Integer: 2 },
    },
  },
  {
    id: "c",
    values: {
      id: { Text: "c" },
      name: { Text: "Alan" },
      status: { Text: "Done" },
      active: { Boolean: true },
      count: { Integer: 3 },
    },
  },
];

describe("viewLayout helpers", () => {
  it("picks primary and subtitle columns for list layout", () => {
    expect(resolveListPrimaryColumn(columns)).toBe("name");
    expect(resolveListSubtitleColumn(columns, "name")).toBe("status");
  });

  it("prefers explicit group_by, then status, then first groupable field", () => {
    expect(resolveGroupByColumn(columns, "active")).toBe("active");
    expect(resolveGroupByColumn(columns, null)).toBe("status");
    expect(
      resolveGroupByColumn(
        columns.filter((column) => column.name !== "status"),
        null,
      ),
    ).toBe("name");
  });

  it("groups rows into sorted lanes", () => {
    const lanes = groupRowsByColumn(rows, "status");
    expect(lanes.map((lane) => lane.key)).toEqual(["Active", "Done"]);
    expect(lanes[0]?.rows.map((row) => row.id)).toEqual(["b"]);
    expect(lanes[1]?.rows.map((row) => row.id)).toEqual(["a", "c"]);
  });
});
