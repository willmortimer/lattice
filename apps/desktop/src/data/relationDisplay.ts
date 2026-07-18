import type { CellValue, DataAppSnapshot, DataColumn, DataRow } from "./types";
import { cellValueToDisplay } from "./types";

/** Map of target table name → record id → display label. */
export type RelationLabelIndex = Map<string, Map<string, string>>;

const NAME_LIKE_FIELDS = ["name", "title", "label"] as const;

/** Record ids stored in a relation cell, or an empty list for null/empty values. */
export function extractRelationIds(value: CellValue | undefined): string[] {
  if (!value || "Null" in value) {
    return [];
  }
  if ("Relation" in value) {
    return [...value.Relation.record_ids];
  }
  return [];
}

/** Canonical relation cell value for IPC round-trips. */
export function relationCellValue(recordIds: readonly string[]): CellValue {
  if (recordIds.length === 0) {
    return { Null: null };
  }
  return { Relation: { record_ids: [...recordIds] } };
}

/** Draft encoding for relation fields in record detail (JSON array of ids). */
export function relationDraftFromIds(recordIds: readonly string[]): string {
  return JSON.stringify([...recordIds]);
}

/** Parse a relation draft string into record ids. Invalid input yields an empty list. */
export function parseRelationDraft(text: string): string[] {
  const trimmed = text.trim();
  if (!trimmed) {
    return [];
  }
  try {
    const parsed: unknown = JSON.parse(trimmed);
    if (!Array.isArray(parsed)) {
      return [];
    }
    return parsed.filter((entry): entry is string => typeof entry === "string");
  } catch {
    return [];
  }
}

export function relationIdsEqual(left: readonly string[], right: readonly string[]): boolean {
  if (left.length !== right.length) {
    return false;
  }
  return left.every((id, index) => id === right[index]);
}

/** Human label for a related row: name-like field, else first text value, else id. */
export function relationRecordLabel(row: DataRow): string {
  for (const field of NAME_LIKE_FIELDS) {
    const value = row.values[field];
    const display = cellValueToDisplay(value);
    if (display) {
      return display;
    }
  }

  for (const [field, value] of Object.entries(row.values)) {
    if (field === "id") {
      continue;
    }
    const display = cellValueToDisplay(value);
    if (display) {
      return display;
    }
  }

  return row.id;
}

export function buildRelationLabelIndex(
  relationTargets: Record<string, DataRow[]> | undefined,
): RelationLabelIndex {
  const index: RelationLabelIndex = new Map();
  if (!relationTargets) {
    return index;
  }

  for (const [table, rows] of Object.entries(relationTargets)) {
    const labels = new Map<string, string>();
    for (const row of rows) {
      labels.set(row.id, relationRecordLabel(row));
    }
    index.set(table, labels);
  }
  return index;
}

function resolveRelationLabel(
  recordId: string,
  targetTable: string | undefined,
  index: RelationLabelIndex,
): string {
  if (!targetTable) {
    return recordId;
  }
  const labels = index.get(targetTable);
  return labels?.get(recordId) ?? recordId;
}

/** Comma-separated linked titles, falling back to raw ids when labels are unavailable. */
export function formatRelationDisplay(
  recordIds: readonly string[],
  targetTable: string | undefined,
  index: RelationLabelIndex,
): string {
  if (recordIds.length === 0) {
    return "";
  }
  return recordIds
    .map((recordId) => resolveRelationLabel(recordId, targetTable, index))
    .join(", ");
}

export function formatRelationCellValue(
  value: CellValue | undefined,
  targetTable: string | undefined,
  index: RelationLabelIndex,
): string {
  return formatRelationDisplay(extractRelationIds(value), targetTable, index);
}

/** Display text for a column cell, resolving relation labels when targets are available. */
export function formatColumnCellDisplay(
  value: CellValue | undefined,
  column: Pick<DataColumn, "field_type" | "relation_table">,
  index: RelationLabelIndex,
): string {
  if (column.field_type === "relation") {
    return formatRelationCellValue(value, column.relation_table, index);
  }
  return cellValueToDisplay(value);
}

/** Display text for a named column on a row (layout views). */
export function formatCellForColumnName(
  row: DataRow,
  columnName: string | undefined,
  columns: DataColumn[],
  index: RelationLabelIndex,
): string {
  if (!columnName) {
    return "";
  }
  const column = columns.find((candidate) => candidate.name === columnName);
  if (!column) {
    return cellValueToDisplay(row.values[columnName]);
  }
  return formatColumnCellDisplay(row.values[columnName], column, index);
}

function relationTargetTables(columns: readonly DataColumn[]): Set<string> {
  const tables = new Set<string>();
  for (const column of columns) {
    if (column.field_type === "relation" && column.relation_table) {
      tables.add(column.relation_table);
    }
  }
  return tables;
}

function cloneRelationTargets(
  targets: Record<string, DataRow[]> | undefined,
): Record<string, DataRow[]> | undefined {
  if (!targets) {
    return undefined;
  }
  return Object.fromEntries(
    Object.entries(targets).map(([table, rows]) => [
      table,
      rows.map((row) => ({ id: row.id, values: { ...row.values } })),
    ]),
  );
}

function upsertRelationTargetRow(
  targets: Record<string, DataRow[]>,
  table: string,
  row: DataRow,
): Record<string, DataRow[]> {
  const existing = targets[table] ?? [];
  const nextRow = { id: row.id, values: { ...row.values } };
  const index = existing.findIndex((candidate) => candidate.id === row.id);
  if (index < 0) {
    return { ...targets, [table]: [...existing, nextRow] };
  }
  return {
    ...targets,
    [table]: existing.map((candidate, candidateIndex) =>
      candidateIndex === index ? nextRow : candidate,
    ),
  };
}

function removeRelationTargetRow(
  targets: Record<string, DataRow[]>,
  table: string,
  rowId: string,
): Record<string, DataRow[]> {
  const existing = targets[table];
  if (!existing) {
    return targets;
  }
  const nextRows = existing.filter((candidate) => candidate.id !== rowId);
  if (nextRows.length === existing.length) {
    return targets;
  }
  if (nextRows.length === 0) {
    const { [table]: _removed, ...rest } = targets;
    return rest;
  }
  return { ...targets, [table]: nextRows };
}

/** Keep `relation_targets` honest after insert/update on the active table. */
export function syncRelationTargetsAfterUpsert(
  snapshot: DataAppSnapshot,
  row: DataRow,
): Record<string, DataRow[]> | undefined {
  const targetTables = relationTargetTables(snapshot.columns);
  if (!targetTables.has(snapshot.default_table)) {
    return snapshot.relation_targets;
  }
  const targets = cloneRelationTargets(snapshot.relation_targets) ?? {};
  return upsertRelationTargetRow(targets, snapshot.default_table, row);
}

/** Keep `relation_targets` honest after delete on the active table. */
export function syncRelationTargetsAfterDelete(
  snapshot: DataAppSnapshot,
  rowId: string,
): Record<string, DataRow[]> | undefined {
  const targetTables = relationTargetTables(snapshot.columns);
  if (!targetTables.has(snapshot.default_table)) {
    return snapshot.relation_targets;
  }
  const targets = cloneRelationTargets(snapshot.relation_targets);
  if (!targets) {
    return undefined;
  }
  return removeRelationTargetRow(targets, snapshot.default_table, rowId);
}
