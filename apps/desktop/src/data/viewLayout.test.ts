import { describe, expect, it } from "vitest";

import type { DataColumn, DataRow } from "./types";
import {
  groupRowsByCalendarDate,
  groupRowsByColumn,
  isImageCoverValue,
  isImageLikeColumn,
  parseCalendarDate,
  resolveCalendarDateColumn,
  resolveGalleryCoverColumn,
  resolveGroupByColumn,
  resolveImageLikeColumn,
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

  it("detects image-like columns and cover values", () => {
    const galleryColumns: DataColumn[] = [
      { name: "id", field_type: "text", sqlite_type: "TEXT" },
      { name: "name", field_type: "text", sqlite_type: "TEXT" },
      { name: "photo", field_type: "text", sqlite_type: "TEXT" },
      { name: "notes", field_type: "long_text", sqlite_type: "TEXT" },
    ];

    expect(isImageLikeColumn(galleryColumns[2]!)).toBe(true);
    expect(isImageLikeColumn(galleryColumns[1]!)).toBe(false);
    expect(resolveImageLikeColumn(galleryColumns)).toBe("photo");
    expect(resolveGalleryCoverColumn(galleryColumns, "notes")).toBe("notes");
    expect(resolveGalleryCoverColumn(galleryColumns, null)).toBe("photo");
    expect(
      resolveGalleryCoverColumn(
        galleryColumns.filter((column) => column.name !== "photo"),
        null,
      ),
    ).toBeUndefined();
  });

  it("recognizes image cover values", () => {
    expect(isImageCoverValue("assets/photo.png")).toBe(true);
    expect(isImageCoverValue("https://example.com/cover.jpg?size=large")).toBe(true);
    expect(isImageCoverValue("data:image/png;base64,abc")).toBe(true);
    expect(isImageCoverValue("Draft notes")).toBe(false);
    expect(isImageCoverValue("https://example.com/page")).toBe(false);
  });

  it("prefers explicit date_field, then date type, then date-like names", () => {
    const calendarColumns: DataColumn[] = [
      { name: "id", field_type: "text", sqlite_type: "TEXT" },
      { name: "name", field_type: "text", sqlite_type: "TEXT" },
      { name: "due_date", field_type: "text", sqlite_type: "TEXT" },
      { name: "created_at", field_type: "date", sqlite_type: "TEXT" },
    ];

    expect(resolveCalendarDateColumn(calendarColumns, "due_date")).toBe("due_date");
    expect(resolveCalendarDateColumn(calendarColumns, null)).toBe("created_at");
    expect(
      resolveCalendarDateColumn(
        calendarColumns.filter((column) => column.name !== "created_at"),
        null,
      ),
    ).toBe("due_date");
  });

  it("parses YYYY-MM-DD and ISO datetimes to calendar dates", () => {
    expect(parseCalendarDate({ Date: "2026-07-04" })).toBe("2026-07-04");
    expect(parseCalendarDate({ Text: "2026-07-04T14:30:00Z" })).toBe("2026-07-04");
    expect(parseCalendarDate({ Text: "2026-13-40" })).toBeUndefined();
    expect(parseCalendarDate({ Text: "soon" })).toBeUndefined();
    expect(parseCalendarDate({ Null: null })).toBeUndefined();
  });

  it("groups rows by calendar date with an undated bucket", () => {
    const datedRows: DataRow[] = [
      {
        id: "a",
        values: {
          id: { Text: "a" },
          due_date: { Date: "2026-07-04" },
        },
      },
      {
        id: "b",
        values: {
          id: { Text: "b" },
          due_date: { Text: "2026-07-04T09:00:00Z" },
        },
      },
      {
        id: "c",
        values: {
          id: { Text: "c" },
          due_date: { Text: "TBD" },
        },
      },
    ];

    const buckets = groupRowsByCalendarDate(datedRows, "due_date");
    expect(buckets.map((bucket) => bucket.key)).toEqual(["2026-07-04", "undated"]);
    expect(buckets[0]?.rows.map((row) => row.id)).toEqual(["a", "b"]);
    expect(buckets[1]?.rows.map((row) => row.id)).toEqual(["c"]);
  });
});
