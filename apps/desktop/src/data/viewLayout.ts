import { cellValueToDisplay, type DataColumn, type DataRow } from "./types";

const GROUPABLE_FIELD_TYPES = new Set(["text", "boolean"]);

/** First editable column shown as the list/board card title. */
export function resolveListPrimaryColumn(columns: DataColumn[]): string | undefined {
  return columns.find((column) => column.name !== "id")?.name;
}

/** Second editable column shown as list subtitle or board card detail. */
export function resolveListSubtitleColumn(
  columns: DataColumn[],
  primary?: string,
): string | undefined {
  return columns.find((column) => column.name !== "id" && column.name !== primary)?.name;
}

/**
 * Board lane column: explicit `group_by` from the view when present and valid,
 * otherwise a column named `status`, otherwise the first text/boolean field.
 */
export function resolveGroupByColumn(
  columns: DataColumn[],
  explicit?: string | null,
): string | undefined {
  if (explicit && columns.some((column) => column.name === explicit)) {
    return explicit;
  }
  const status = columns.find((column) => column.name === "status");
  if (status) {
    return status.name;
  }
  return columns.find(
    (column) => column.name !== "id" && GROUPABLE_FIELD_TYPES.has(column.field_type),
  )?.name;
}

export interface BoardLane {
  key: string;
  rows: DataRow[];
}

export function groupRowsByColumn(rows: DataRow[], column: string): BoardLane[] {
  const buckets = new Map<string, DataRow[]>();
  for (const row of rows) {
    const key = cellValueToDisplay(row.values[column]) || "(empty)";
    const bucket = buckets.get(key);
    if (bucket) {
      bucket.push(row);
    } else {
      buckets.set(key, [row]);
    }
  }
  return [...buckets.entries()]
    .sort(([left], [right]) => left.localeCompare(right, undefined, { numeric: true }))
    .map(([key, laneRows]) => ({ key, rows: laneRows }));
}
