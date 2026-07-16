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

export interface DataAppSnapshot {
  title: string;
  default_table: string;
  package_revision: string;
  columns: DataColumn[];
  rows: DataRow[];
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
  };
}
