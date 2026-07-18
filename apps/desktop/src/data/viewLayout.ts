import { cellValueToDisplay, type DataColumn, type DataRow } from "./types";

const GROUPABLE_FIELD_TYPES = new Set(["text", "boolean"]);

const IMAGE_LIKE_COLUMN_PATTERN =
  /^(?:photo|image|cover|thumbnail|thumb|picture|avatar|poster|icon|banner)(?:_|$)/i;

const IMAGE_COVER_VALUE_PATTERN = /\.(png|jpe?g|gif|webp|avif|bmp|tiff|svg)(?:[?#].*)?$/i;

/** First editable column shown as the list/board card title. */
export function resolveListPrimaryColumn(columns: DataColumn[]): string | undefined {
  return columns.find((column) => column.name !== "id")?.name;
}

/** Column names commonly used for gallery cover images. */
export function isImageLikeColumn(column: DataColumn): boolean {
  if (column.name === "id") {
    return false;
  }
  return (
    (column.field_type === "text" || column.field_type === "long_text") &&
    IMAGE_LIKE_COLUMN_PATTERN.test(column.name)
  );
}

/** First text column whose name suggests it stores an image reference. */
export function resolveImageLikeColumn(columns: DataColumn[]): string | undefined {
  return columns.find((column) => isImageLikeColumn(column))?.name;
}

/**
 * Gallery cover column: explicit `cover_field` when present and valid,
 * otherwise the first image-like column.
 */
export function resolveGalleryCoverColumn(
  columns: DataColumn[],
  explicit?: string | null,
): string | undefined {
  if (explicit && columns.some((column) => column.name === explicit)) {
    return explicit;
  }
  return resolveImageLikeColumn(columns);
}

/** Whether a cell value looks like an image URL or workspace path. */
export function isImageCoverValue(value: string): boolean {
  const trimmed = value.trim();
  if (!trimmed) {
    return false;
  }
  if (/^data:image\//i.test(trimmed)) {
    return true;
  }
  if (/^https?:\/\//i.test(trimmed)) {
    return IMAGE_COVER_VALUE_PATTERN.test(trimmed);
  }
  return IMAGE_COVER_VALUE_PATTERN.test(trimmed);
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
