import {
  cellValueToDisplay,
  type CellValue,
  type DataColumn,
  type DataRow,
  type LayoutSummary,
  type ViewLayoutType,
} from "./types";

export const VIEW_LAYOUT_TYPES: readonly ViewLayoutType[] = [
  "grid",
  "list",
  "board",
  "gallery",
  "calendar",
  "form",
] as const;

/** Layout fields included in `save_data_view` (layout-specific keys only when applicable). */
export interface ViewLayoutSaveFields {
  layoutType: ViewLayoutType;
  groupBy?: string | null;
  coverField?: string | null;
  dateField?: string | null;
  summaries?: LayoutSummary[];
}

/**
 * Build the layout portion of a save-view request.
 * Only the active layout's field is included so YAML stays valid.
 */
export function layoutFieldsForSave(
  layoutType: ViewLayoutType,
  fields: {
    groupBy?: string;
    coverField?: string;
    dateField?: string;
    summaries?: LayoutSummary[];
  },
): ViewLayoutSaveFields {
  switch (layoutType) {
    case "grid":
      return {
        layoutType,
        groupBy: fields.groupBy ?? null,
        summaries: fields.summaries,
      };
    case "list":
    case "form":
      return { layoutType };
    case "board":
      return { layoutType, groupBy: fields.groupBy ?? null };
    case "gallery":
      return { layoutType, coverField: fields.coverField ?? null };
    case "calendar":
      return { layoutType, dateField: fields.dateField ?? null };
    default: {
      const _exhaustive: never = layoutType;
      return _exhaustive;
    }
  }
}

/**
 * When the user picks a layout in the toolbar, seed layout-specific columns
 * from resolvers so Save persists an explicit field.
 */
export function seedLayoutFieldsForType(
  layoutType: ViewLayoutType,
  columns: DataColumn[],
  current: {
    groupBy?: string;
    coverField?: string;
    dateField?: string;
  },
): {
  groupBy?: string;
  coverField?: string;
  dateField?: string;
} {
  switch (layoutType) {
    case "grid":
      // Optional grouping only — do not invent a board-style default.
      return {
        ...current,
        groupBy:
          current.groupBy && columns.some((column) => column.name === current.groupBy)
            ? current.groupBy
            : undefined,
      };
    case "list":
    case "form":
      return current;
    case "board":
      return {
        ...current,
        groupBy: resolveGroupByColumn(columns, current.groupBy),
      };
    case "gallery":
      return {
        ...current,
        coverField: resolveGalleryCoverColumn(columns, current.coverField),
      };
    case "calendar":
      return {
        ...current,
        dateField: resolveCalendarDateColumn(columns, current.dateField),
      };
    default: {
      const _exhaustive: never = layoutType;
      return _exhaustive;
    }
  }
}

const GROUPABLE_FIELD_TYPES = new Set(["text", "boolean"]);

export type LayoutFieldPickerKind = "groupBy" | "coverField" | "dateField";

export interface LayoutFieldPickerSpec {
  kind: LayoutFieldPickerKind;
  label: string;
  ariaLabel: string;
  options: DataColumn[];
}

function columnsMatchingPicker(
  columns: DataColumn[],
  predicate: (column: DataColumn) => boolean,
  explicit?: string | null,
): DataColumn[] {
  const matches = columns.filter((column) => column.name !== "id" && predicate(column));
  if (explicit && !matches.some((column) => column.name === explicit)) {
    const extra = columns.find((column) => column.name === explicit);
    if (extra) {
      return [extra, ...matches];
    }
  }
  return matches;
}

/** Columns eligible for board `group_by` picker. */
export function groupableColumnsForPicker(
  columns: DataColumn[],
  explicit?: string | null,
): DataColumn[] {
  return columnsMatchingPicker(
    columns,
    (column) => GROUPABLE_FIELD_TYPES.has(column.field_type),
    explicit,
  );
}

/** Columns eligible for gallery `cover_field` picker. */
export function coverColumnsForPicker(
  columns: DataColumn[],
  explicit?: string | null,
): DataColumn[] {
  return columnsMatchingPicker(columns, () => true, explicit);
}

/** Columns eligible for calendar `date_field` picker. */
export function dateColumnsForPicker(
  columns: DataColumn[],
  explicit?: string | null,
): DataColumn[] {
  return columnsMatchingPicker(
    columns,
    (column) =>
      column.field_type === "date" || DATE_LIKE_COLUMN_PATTERN.test(column.name),
    explicit,
  );
}

/** Active layout field pickers for the toolbar (empty when layout has no layout field). */
export function layoutFieldPickerSpecs(
  layoutType: ViewLayoutType,
  columns: DataColumn[],
  current: {
    groupBy?: string;
    coverField?: string;
    dateField?: string;
  },
): LayoutFieldPickerSpec[] {
  switch (layoutType) {
    case "grid":
      return [
        {
          kind: "groupBy",
          label: "Group by",
          ariaLabel: "Grid group by column",
          options: groupableColumnsForPicker(columns, current.groupBy),
        },
      ];
    case "list":
    case "form":
      return [];
    case "board":
      return [
        {
          kind: "groupBy",
          label: "Group by",
          ariaLabel: "Board group by column",
          options: groupableColumnsForPicker(columns, current.groupBy),
        },
      ];
    case "gallery":
      return [
        {
          kind: "coverField",
          label: "Cover",
          ariaLabel: "Gallery cover column",
          options: coverColumnsForPicker(columns, current.coverField),
        },
      ];
    case "calendar":
      return [
        {
          kind: "dateField",
          label: "Date",
          ariaLabel: "Calendar date column",
          options: dateColumnsForPicker(columns, current.dateField),
        },
      ];
    default: {
      const _exhaustive: never = layoutType;
      return _exhaustive;
    }
  }
}

export function layoutFieldPickerValue(
  kind: LayoutFieldPickerKind,
  fields: {
    groupBy?: string;
    coverField?: string;
    dateField?: string;
  },
): string | undefined {
  switch (kind) {
    case "groupBy":
      return fields.groupBy;
    case "coverField":
      return fields.coverField;
    case "dateField":
      return fields.dateField;
    default: {
      const _exhaustive: never = kind;
      return _exhaustive;
    }
  }
}

const ISO_DATE_PREFIX = /^(\d{4}-\d{2}-\d{2})/;

const DATE_LIKE_COLUMN_PATTERN =
  /^(?:date|due_date|due|start_date|start|end_date|end|scheduled|deadline|created_at|updated_at|timestamp)(?:_|$)/i;

const IMAGE_LIKE_COLUMN_PATTERN =
  /^(?:photo|image|cover|thumbnail|thumb|picture|avatar|poster|icon|banner)(?:_|$)/i;

const IMAGE_COVER_VALUE_PATTERN = /\.(png|jpe?g|gif|webp|avif|bmp|tiff|svg)(?:[?#].*)?$/i;

/** First editable column shown as the list/board card title. */
export function resolveListPrimaryColumn(columns: DataColumn[]): string | undefined {
  return columns.find((column) => column.name !== "id")?.name;
}

/**
 * Form field columns: explicit `layout.columns` order when non-empty (excluding `id`),
 * otherwise all non-`id` columns in table order.
 */
export function resolveFormColumns(
  columns: DataColumn[],
  columnOrder: readonly string[] = [],
): DataColumn[] {
  const byName = new Map(columns.map((column) => [column.name, column]));

  if (columnOrder.length > 0) {
    const resolved: DataColumn[] = [];
    for (const name of columnOrder) {
      if (name === "id") {
        continue;
      }
      const column = byName.get(name);
      if (column) {
        resolved.push(column);
      }
    }
    return resolved;
  }

  return columns.filter((column) => column.name !== "id");
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

/**
 * Calendar date column: explicit `date_field` when present and valid,
 * otherwise the first `date` field type, otherwise a date-like column name.
 */
export function resolveCalendarDateColumn(
  columns: DataColumn[],
  explicit?: string | null,
): string | undefined {
  if (explicit && columns.some((column) => column.name === explicit)) {
    return explicit;
  }
  const typed = columns.find(
    (column) => column.name !== "id" && column.field_type === "date",
  );
  if (typed) {
    return typed.name;
  }
  return columns.find(
    (column) => column.name !== "id" && DATE_LIKE_COLUMN_PATTERN.test(column.name),
  )?.name;
}

/** Parse a cell value to `YYYY-MM-DD`, or `undefined` when unparseable. */
export function parseCalendarDate(value: CellValue | undefined): string | undefined {
  const raw = cellValueToDisplay(value).trim();
  if (!raw) {
    return undefined;
  }
  const match = raw.match(ISO_DATE_PREFIX);
  if (!match) {
    return undefined;
  }
  const isoDate = match[1]!;
  const [year, month, day] = isoDate.split("-").map((part) => Number.parseInt(part, 10));
  if (!year || !month || !day) {
    return undefined;
  }
  const probe = new Date(Date.UTC(year, month - 1, day));
  if (
    probe.getUTCFullYear() !== year ||
    probe.getUTCMonth() !== month - 1 ||
    probe.getUTCDate() !== day
  ) {
    return undefined;
  }
  return isoDate;
}

export interface CalendarDayBucket {
  /** `YYYY-MM-DD` for dated rows; `undated` for rows without a parseable date. */
  key: string;
  rows: DataRow[];
}

/** Group rows by calendar date (`YYYY-MM-DD`) or an undated bucket. */
export function groupRowsByCalendarDate(rows: DataRow[], dateColumn: string): CalendarDayBucket[] {
  const dated = new Map<string, DataRow[]>();
  const undated: DataRow[] = [];
  for (const row of rows) {
    const isoDate = parseCalendarDate(row.values[dateColumn]);
    if (!isoDate) {
      undated.push(row);
      continue;
    }
    const bucket = dated.get(isoDate);
    if (bucket) {
      bucket.push(row);
    } else {
      dated.set(isoDate, [row]);
    }
  }
  const buckets: CalendarDayBucket[] = [...dated.entries()]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([key, bucketRows]) => ({ key, rows: bucketRows }));
  if (undated.length > 0) {
    buckets.push({ key: "undated", rows: undated });
  }
  return buckets;
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
