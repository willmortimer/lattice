/**
 * Arrow IPC transport helpers for analytical query results (ADR 0021).
 *
 * The full batch stays as columnar IPC bytes (`ipcBytes`). Schema and a tiny
 * preview come from JSON control fields so the UI can dump shape + samples
 * without building per-cell JavaScript objects for the entire batch.
 */

export interface ArrowFieldMeta {
  name: string;
  dataType: string;
  nullable: boolean;
}

export interface ArrowSchemaMeta {
  fields: ArrowFieldMeta[];
}

export interface ArrowQueryResult {
  schemaMeta: ArrowSchemaMeta;
  /** Raw Arrow IPC stream. Tauri may deliver `number[]` or `Uint8Array`. */
  ipcBytes: Uint8Array | number[] | ArrayBuffer;
  rowCount: number;
  truncated: boolean;
  cancelled: boolean;
  byteLength: number;
  /** Bounded preview only (default ≤5 rows). Never expand `ipcBytes` into this. */
  sampleRows?: unknown[][];
  sql?: string;
}

export interface ArrowDumpOptions {
  /** Max sample rows to include in the dump (default 5). */
  sampleRows?: number;
}

export interface ArrowTransportDump {
  schema: ArrowFieldMeta[];
  rowCount: number;
  truncated: boolean;
  cancelled: boolean;
  byteLength: number;
  ipcByteLength: number;
  /** Bounded preview only — never the full batch as row objects. */
  sampleRows: unknown[][];
  sql?: string;
}

/** Normalize Tauri binary payloads to `Uint8Array` without copying when possible. */
export function ipcBytesToUint8Array(
  bytes: Uint8Array | number[] | ArrayBuffer,
): Uint8Array {
  if (bytes instanceof Uint8Array) return bytes;
  if (bytes instanceof ArrayBuffer) return new Uint8Array(bytes);
  return Uint8Array.from(bytes);
}

/**
 * Detached `ArrayBuffer` for Arrow-native consumers (Perspective `worker.table`).
 * Always copies so sliced `Uint8Array` views do not share a larger backing store.
 */
export function ipcBytesToArrayBuffer(
  bytes: Uint8Array | number[] | ArrayBuffer,
): ArrayBuffer {
  if (bytes instanceof ArrayBuffer) return bytes.slice(0);
  const view = bytes instanceof Uint8Array ? bytes : Uint8Array.from(bytes);
  const copy = new Uint8Array(view.byteLength);
  copy.set(view);
  return copy.buffer;
}

/**
 * Dump schema + a small sample from an Arrow IPC transport payload.
 *
 * Does not decode the full IPC batch into row objects. `ipcBytes` remain available
 * for Arrow-native consumers (Perspective, workers) while the dump uses control
 * metadata only.
 */
export function dumpArrowTransport(
  result: ArrowQueryResult,
  options: ArrowDumpOptions = {},
): ArrowTransportDump {
  const sampleLimit = Math.max(0, options.sampleRows ?? 5);
  const ipc = ipcBytesToUint8Array(result.ipcBytes);
  const preview = result.sampleRows ?? [];
  const sampleRows = preview.slice(0, Math.min(sampleLimit, preview.length));

  return {
    schema: result.schemaMeta.fields,
    rowCount: result.rowCount,
    truncated: result.truncated,
    cancelled: result.cancelled,
    byteLength: result.byteLength,
    ipcByteLength: ipc.byteLength,
    sampleRows,
    sql: result.sql,
  };
}

/** Human-readable one-liner for dataset placeholder UI. */
export function formatArrowDumpSummary(dump: ArrowTransportDump): string {
  const schema = dump.schema.map((field) => `${field.name}:${field.dataType}`).join(", ");
  const truncated = dump.truncated ? " (truncated)" : "";
  const schemaPart = schema.length > 0 ? schema : "(empty)";
  return `Ran query → ${dump.rowCount} rows${truncated}, schema: ${schemaPart}`;
}
