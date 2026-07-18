/** Mirrors `lattice_data::FieldType` (snake_case in JSON from Rust). */
export type FieldType =
  | "text"
  | "long_text"
  | "integer"
  | "decimal"
  | "boolean"
  | "date";

/** Externally tagged `CellValue` from `lattice-data`. */
export type CellValue =
  | { Null: null }
  | { Text: string }
  | { Integer: number }
  | { Decimal: number }
  | { Boolean: boolean }
  | { Date: string };

export interface DataColumn {
  name: string;
  field_type: FieldType;
  sqlite_type: string;
}

export interface DataRow {
  id: string;
  values: Record<string, CellValue>;
}

export interface ViewFilter {
  field: string;
  operator: "equals" | "contains";
  value: string;
}

export type ViewLayoutType = "grid" | "list" | "board" | "gallery" | "calendar" | "form";

export interface DataAppSnapshot {
  title: string;
  default_table: string;
  package_revision: string;
  columns: DataColumn[];
  rows: DataRow[];
  available_views: string[];
  active_view: string;
  sort_field?: string;
  sort_direction?: "asc" | "desc";
  filters: ViewFilter[];
  /** Active view layout: `grid`, `list`, `board`, `gallery`, `calendar`, or `form`. */
  layout_type: ViewLayoutType;
  /** Board layout: explicit group-by column from the view YAML. */
  group_by?: string;
  /** Gallery layout: column rendered as each card's cover. */
  cover_field?: string;
  /** Calendar layout: column used to place records on the calendar. */
  date_field?: string;
}

export function cellValueToDisplay(value: CellValue | undefined): string {
  if (!value) return "";
  if ("Null" in value) return "";
  if ("Text" in value) return value.Text;
  if ("Integer" in value) return String(value.Integer);
  if ("Decimal" in value) return String(value.Decimal);
  if ("Boolean" in value) return value.Boolean ? "true" : "false";
  if ("Date" in value) return value.Date;
  return "";
}

export function displayToCellValue(text: string, fieldType: FieldType): CellValue {
  const trimmed = text.trim();
  if (!trimmed) return { Null: null };
  switch (fieldType) {
    case "integer":
      return { Integer: Number.parseInt(trimmed, 10) };
    case "decimal":
      return { Decimal: Number.parseFloat(trimmed) };
    case "boolean":
      return {
        Boolean: ["1", "true", "yes"].includes(trimmed.toLowerCase()),
      };
    case "date":
      return { Date: trimmed };
    case "text":
    case "long_text":
    default:
      return { Text: text };
  }
}

export function cloneSnapshot(snapshot: DataAppSnapshot): DataAppSnapshot {
  return {
    ...snapshot,
    columns: snapshot.columns.map((column) => ({ ...column })),
    rows: snapshot.rows.map((row) => ({
      id: row.id,
      values: { ...row.values },
    })),
    available_views: [...snapshot.available_views],
    filters: snapshot.filters.map((filter) => ({ ...filter })),
    layout_type: snapshot.layout_type,
    group_by: snapshot.group_by,
    cover_field: snapshot.cover_field,
    date_field: snapshot.date_field,
  };
}
