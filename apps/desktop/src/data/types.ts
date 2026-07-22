import type { ConditionalFormatRule } from "./conditionalFormat";

export type { ConditionalFormatRule, ConditionalFormatStyle } from "./conditionalFormat";

/** Mirrors `lattice_data::FieldType` (snake_case in JSON from Rust). */
export type FieldType =
  | "text"
  | "long_text"
  | "integer"
  | "decimal"
  | "boolean"
  | "date"
  | "relation"
  | "lookup"
  | "rollup"
  | "formula";

/** Mirrors `lattice_data::RollupAggregate`. */
export type RollupAggregate = "count" | "sum" | "min" | "max";

/** Mirrors `lattice_data::FormulaValue`. */
export type FormulaValue = { Number: number } | { Text: string };

/** Externally tagged `CellValue` from `lattice-data`. */
export type CellValue =
  | { Null: null }
  | { Text: string }
  | { Integer: number }
  | { Decimal: number }
  | { Boolean: boolean }
  | { Date: string }
  | { Relation: { record_ids: string[] } }
  | { Lookup: { values: string[] } }
  | { Rollup: { value: number | null } }
  | { Formula: { value: FormulaValue | null } };

export interface DataColumn {
  name: string;
  field_type: FieldType;
  sqlite_type: string;
  /** Target table for relation fields (same `.data` package). */
  relation_table?: string;
  /** Optional junction table for M2M relation storage (demo opt-in). */
  junction_table?: string;
  /** Source relation column for lookup fields. */
  lookup_relation?: string;
  /** Related-table field projected by lookup fields. */
  lookup_field?: string;
  /** Source relation column for rollup fields. */
  rollup_relation?: string;
  /** Aggregate for rollup fields. */
  rollup_aggregate?: RollupAggregate;
  /** Related-table field aggregated by rollup fields. */
  rollup_field?: string;
  /** Expression for formula fields (e.g. `{price} * {quantity}`). */
  formula?: string;
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

/** Saved view layout metadata for browser demo fixtures (from template seeds). */
export interface DataViewSnapshot {
  name: string;
  layout_type: ViewLayoutType;
  group_by?: string;
  cover_field?: string;
  date_field?: string;
}

export interface DataAppSnapshot {
  title: string;
  default_table: string;
  package_revision: string;
  columns: DataColumn[];
  rows: DataRow[];
  /** 0-based start of the `rows` window. */
  row_offset: number;
  /** Requested max rows for this window. */
  row_limit: number;
  /** Total matching rows after view filters (not just this window). */
  row_total: number;
  /** True when `row_offset + rows.length < row_total`. */
  has_more: boolean;
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
  /** View-scoped conditional format rules applied to matching grid cells. */
  conditional_format?: ConditionalFormatRule[];
  /** Browser demo: per-view layout metadata from template `dataPackages[].views`. */
  saved_views?: DataViewSnapshot[];
  /** Rows from tables referenced by relation columns (for picker labels). */
  relation_targets?: Record<string, DataRow[]>;
}

/**
 * Display string for a cell. Tolerates Rust externally-tagged IPC shapes,
 * including unit `Null` serialized as the JSON string `"Null"` (not `{Null:null}`).
 */
export function cellValueToDisplay(value: CellValue | undefined | null | string): string {
  if (value == null || value === "" || value === "Null") return "";
  if (typeof value !== "object") return String(value);
  if ("Null" in value) return "";
  if ("Text" in value) return value.Text ?? "";
  if ("Integer" in value) return String(value.Integer);
  if ("Decimal" in value) return String(value.Decimal);
  if ("Boolean" in value) return value.Boolean ? "true" : "false";
  if ("Date" in value) return value.Date ?? "";
  if ("Relation" in value) {
    const ids = value.Relation?.record_ids;
    return Array.isArray(ids) ? ids.join(", ") : "";
  }
  if ("Lookup" in value) {
    const values = value.Lookup?.values;
    return Array.isArray(values) ? values.join(", ") : "";
  }
  if ("Rollup" in value) {
    const rollupValue = value.Rollup?.value;
    if (rollupValue == null) return "";
    return String(rollupValue);
  }
  if ("Formula" in value) {
    const formulaValue = value.Formula?.value;
    if (formulaValue == null) return "";
    if ("Number" in formulaValue) return String(formulaValue.Number);
    if ("Text" in formulaValue) return formulaValue.Text ?? "";
    return "";
  }
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
    case "relation":
      return {
        Relation: {
          record_ids: trimmed
            .split(",")
            .map((id) => id.trim())
            .filter(Boolean),
        },
      };
    case "lookup":
      return {
        Lookup: {
          values: trimmed
            .split(",")
            .map((part) => part.trim())
            .filter(Boolean),
        },
      };
    case "rollup": {
      if (!trimmed) return { Rollup: { value: null } };
      const parsed = Number.parseFloat(trimmed);
      return { Rollup: { value: Number.isNaN(parsed) ? null : parsed } };
    }
    case "formula": {
      if (!trimmed) return { Formula: { value: null } };
      const parsed = Number.parseFloat(trimmed);
      if (!Number.isNaN(parsed) && String(parsed) === trimmed) {
        return { Formula: { value: { Number: parsed } } };
      }
      return { Formula: { value: { Text: text } } };
    }
    case "text":
    case "long_text":
      return { Text: text };
    default: {
      const _exhaustive: never = fieldType;
      return _exhaustive;
    }
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
    conditional_format: snapshot.conditional_format?.map((rule) => ({
      ...rule,
      style: { ...rule.style },
    })),
    saved_views: snapshot.saved_views?.map((view) => ({ ...view })),
    relation_targets: snapshot.relation_targets
      ? Object.fromEntries(
          Object.entries(snapshot.relation_targets).map(([table, rows]) => [
            table,
            rows.map((row) => ({
              id: row.id,
              values: { ...row.values },
            })),
          ]),
        )
      : undefined,
  };
}
