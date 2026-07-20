import { describe, expect, it } from "vitest";
import {
  dumpArrowTransport,
  formatArrowDumpSummary,
  ipcBytesToArrayBuffer,
  ipcBytesToUint8Array,
  type ArrowQueryResult,
} from "./arrowIpc";

function fixture(overrides: Partial<ArrowQueryResult> = {}): ArrowQueryResult {
  return {
    schemaMeta: {
      fields: [
        { name: "id", dataType: "int64", nullable: false },
        { name: "name", dataType: "utf8", nullable: true },
      ],
    },
    ipcBytes: [1, 2, 3, 4, 5],
    rowCount: 100,
    truncated: true,
    cancelled: false,
    byteLength: 5,
    sampleRows: [
      [1, "Ada"],
      [2, "Grace"],
      [3, "Alan"],
    ],
    sql: "SELECT * FROM facts",
    ...overrides,
  };
}

describe("arrowIpc", () => {
  it("normalizes ipc bytes without inventing row objects for the full batch", () => {
    const result = fixture();
    const dump = dumpArrowTransport(result, { sampleRows: 2 });
    expect(dump.rowCount).toBe(100);
    expect(dump.truncated).toBe(true);
    expect(dump.schema).toEqual(result.schemaMeta.fields);
    expect(dump.sampleRows).toEqual([
      [1, "Ada"],
      [2, "Grace"],
    ]);
    expect(dump.ipcByteLength).toBe(5);
    // Full batch stays binary — dump never expands 100 rows.
    expect(dump.sampleRows.length).toBeLessThan(result.rowCount);
  });

  it("formats a dataset placeholder summary", () => {
    const dump = dumpArrowTransport(fixture());
    expect(formatArrowDumpSummary(dump)).toBe(
      "Ran query → 100 rows (truncated), schema: id:int64, name:utf8",
    );
  });

  it("accepts Uint8Array ipc payloads from Tauri", () => {
    const bytes = new Uint8Array([9, 8, 7]);
    const dump = dumpArrowTransport(fixture({ ipcBytes: bytes, byteLength: 3 }));
    expect(ipcBytesToUint8Array(bytes)).toEqual(bytes);
    expect(dump.ipcByteLength).toBe(3);
  });

  it("copies ipc into a detached ArrayBuffer for Perspective", () => {
    const sliced = new Uint8Array([0, 10, 20, 30]).subarray(1, 3);
    const buffer = ipcBytesToArrayBuffer(sliced);
    expect(new Uint8Array(buffer)).toEqual(new Uint8Array([10, 20]));
  });
});
