import { describe, expect, it, vi } from "vitest";
import { PDF_RANGE_CHUNK_SIZE } from "./mediaLimits";
import { createPdfDataRangeTransport, readPdfRangeInChunks } from "./pdfRangeTransport";

describe("PDF range transport", () => {
  it("never asks the native reader for more than 256 KiB", async () => {
    const reads: Array<{ offset: number; length: number }> = [];
    await readPdfRangeInChunks(3, PDF_RANGE_CHUNK_SIZE * 2 + 7, {
      read: vi.fn(async (offset, length) => {
        reads.push({ offset, length });
        return new Uint8Array(length);
      }),
    }, new AbortController().signal);
    expect(reads.map((read) => read.length)).toEqual([PDF_RANGE_CHUNK_SIZE, PDF_RANGE_CHUNK_SIZE, 4]);
    expect(reads[0].offset).toBe(3);
  });

  it("forwards each bounded chunk to PDF.js", async () => {
    class FakeTransport {
      ranges: Array<{ begin: number; length: number }> = [];
      aborted = false;
      constructor(public length: number, _initial: Uint8Array | null, _progressiveDone?: boolean) {}
      onDataRange = (begin: number, chunk: Uint8Array) => this.ranges.push({ begin, length: chunk.byteLength });
      requestDataRange(_begin: number, _end: number): void {}
      abort = () => { this.aborted = true; };
    }
    const transport = createPdfDataRangeTransport(FakeTransport, PDF_RANGE_CHUNK_SIZE * 2 + 1, {
      read: async (_offset, length) => new Uint8Array(length),
    }, new AbortController().signal);
    transport.requestDataRange(0, PDF_RANGE_CHUNK_SIZE * 2 + 1);
    await new Promise((resolve) => setTimeout(resolve, 10));
    expect(transport.ranges.map((range) => range.length)).toEqual([PDF_RANGE_CHUNK_SIZE, PDF_RANGE_CHUNK_SIZE, 1]);
  });
});
