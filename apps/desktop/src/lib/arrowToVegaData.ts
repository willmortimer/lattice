import { tableFromIPC, type Table } from "apache-arrow";

import { ipcBytesToUint8Array, type ArrowFieldMeta, type ArrowQueryResult } from "./arrowIpc";

export interface VegaRow {
  [column: string]: unknown;
}

/** Decode bounded Arrow IPC bytes into plain row objects for Vega-Lite. */
export function arrowIpcToValues(
  ipcBytes: ArrowQueryResult["ipcBytes"],
  maxRows?: number,
): VegaRow[] {
  const bytes = ipcBytesToUint8Array(ipcBytes);
  if (bytes.byteLength === 0) return [];
  const table = tableFromIPC(bytes);
  return tableToValues(table, maxRows);
}

/** Convert preview rows + schema metadata when IPC decode is unnecessary. */
export function sampleRowsToValues(
  sampleRows: unknown[][],
  schema: ArrowFieldMeta[],
): VegaRow[] {
  if (sampleRows.length === 0 || schema.length === 0) return [];
  return sampleRows.map((row) => {
    const record: VegaRow = {};
    for (let index = 0; index < schema.length; index += 1) {
      record[schema[index]?.name ?? `column_${index}`] = row[index] ?? null;
    }
    return record;
  });
}

export function queryResultToValues(result: ArrowQueryResult, maxRows?: number): VegaRow[] {
  try {
    const values = arrowIpcToValues(result.ipcBytes, maxRows);
    if (values.length > 0) return values;
  } catch {
    // Fall back to the bounded JSON preview when IPC decode fails in tests or stubs.
  }
  return sampleRowsToValues(result.sampleRows ?? [], result.schemaMeta.fields);
}

function tableToValues(table: Table, maxRows?: number): VegaRow[] {
  const fields = table.schema.fields;
  if (fields.length === 0 || table.numRows === 0) return [];

  const columns = fields.map((field) => ({
    name: field.name,
    vector: table.getChild(field.name),
  }));
  const limit = maxRows === undefined ? table.numRows : Math.min(maxRows, table.numRows);
  const rows: VegaRow[] = [];

  for (let rowIndex = 0; rowIndex < limit; rowIndex += 1) {
    const record: VegaRow = {};
    for (const column of columns) {
      record[column.name] = column.vector?.get(rowIndex) ?? null;
    }
    rows.push(record);
  }

  return rows;
}
