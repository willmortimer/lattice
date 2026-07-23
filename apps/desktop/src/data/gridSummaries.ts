import {
  cellValueToDisplay,
  type CellValue,
  type DataColumn,
  type DataRow,
  type FieldType,
  type RollupAggregate,
} from "./types";
import { groupRowsByColumn } from "./viewLayout";

export interface LayoutSummary {
  field: string;
  aggregate: RollupAggregate;
}

export type GridRowKind = "data" | "group-header" | "group-footer" | "grand-footer";

export interface GridDisplayRow {
  kind: GridRowKind;
  dataRow?: DataRow;
  groupKey?: string;
  groupCount?: number;
  summaryValues?: Record<string, string>;
}

function cellValueIsEmpty(value: CellValue | undefined): boolean {
  if (!value || "Null" in value) return true;
  const display = cellValueToDisplay(value);
  return display.trim() === "";
}

function numericCellValue(value: CellValue | undefined, fieldType: FieldType): number | undefined {
  if (!value) return undefined;
  if ("Integer" in value) return value.Integer;
  if ("Decimal" in value) return value.Decimal;
  if ("Rollup" in value) {
    const rollupValue = value.Rollup?.value;
    return rollupValue == null ? undefined : rollupValue;
  }
  if ("Formula" in value) {
    const formulaValue = value.Formula?.value;
    if (formulaValue && "Number" in formulaValue) {
      return formulaValue.Number;
    }
  }
  if (fieldType === "integer" || fieldType === "decimal") {
    const parsed = Number.parseFloat(cellValueToDisplay(value));
    return Number.isNaN(parsed) ? undefined : parsed;
  }
  return undefined;
}

export function computeLayoutSummary(
  rows: readonly DataRow[],
  field: string,
  fieldType: FieldType,
  aggregate: RollupAggregate,
): number | undefined {
  switch (aggregate) {
    case "count":
      return rows.filter((row) => !cellValueIsEmpty(row.values[field])).length;
    case "sum":
    case "min":
    case "max": {
      const numbers = rows
        .map((row) => numericCellValue(row.values[field], fieldType))
        .filter((value): value is number => value !== undefined);
      if (numbers.length === 0) return undefined;
      switch (aggregate) {
        case "sum":
          return numbers.reduce((total, value) => total + value, 0);
        case "min":
          return Math.min(...numbers);
        case "max":
          return Math.max(...numbers);
        default: {
          const _exhaustive: never = aggregate;
          return _exhaustive;
        }
      }
    }
    default: {
      const _exhaustive: never = aggregate;
      return _exhaustive;
    }
  }
}

function formatSummaryValue(value: number | undefined, aggregate: RollupAggregate): string {
  if (value === undefined) return "";
  if (aggregate === "count") {
    return String(Math.trunc(value));
  }
  return Number.isInteger(value) ? String(value) : String(Number(value.toFixed(2)));
}

export function summaryValuesForRows(
  rows: readonly DataRow[],
  columns: readonly DataColumn[],
  summaries: readonly LayoutSummary[],
): Record<string, string> {
  const columnByName = new Map(columns.map((column) => [column.name, column]));
  const values: Record<string, string> = {};
  for (const summary of summaries) {
    const column = columnByName.get(summary.field);
    const computed = computeLayoutSummary(
      rows,
      summary.field,
      column?.field_type ?? "text",
      summary.aggregate,
    );
    values[summary.field] = formatSummaryValue(computed, summary.aggregate);
  }
  return values;
}

export function buildGroupedGridRows(
  rows: readonly DataRow[],
  columns: readonly DataColumn[],
  groupBy: string | undefined,
  summaries: readonly LayoutSummary[],
): GridDisplayRow[] {
  if (!groupBy && summaries.length === 0) {
    return rows.map((dataRow) => ({ kind: "data", dataRow }));
  }

  const display: GridDisplayRow[] = [];

  if (groupBy) {
    for (const lane of groupRowsByColumn([...rows], groupBy)) {
      display.push({
        kind: "group-header",
        groupKey: lane.key,
        groupCount: lane.rows.length,
      });
      for (const dataRow of lane.rows) {
        display.push({ kind: "data", dataRow });
      }
      if (summaries.length > 0) {
        display.push({
          kind: "group-footer",
          groupKey: lane.key,
          summaryValues: summaryValuesForRows(lane.rows, columns, summaries),
        });
      }
    }
  } else {
    for (const dataRow of rows) {
      display.push({ kind: "data", dataRow });
    }
  }

  if (summaries.length > 0) {
    display.push({
      kind: "grand-footer",
      summaryValues: summaryValuesForRows(rows, columns, summaries),
    });
  }

  return display;
}

export function dataRowAtGridIndex(
  gridRows: readonly GridDisplayRow[],
  rowIndex: number,
): DataRow | undefined {
  const row = gridRows[rowIndex];
  return row?.kind === "data" ? row.dataRow : undefined;
}
