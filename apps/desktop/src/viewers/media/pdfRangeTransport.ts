import { readResourceRange, type ResourceLocation } from "../../lib/resourceRuntime";
import { PDF_RANGE_CHUNK_SIZE } from "./mediaLimits";

export interface PdfRangeReader {
  read: (offset: number, length: number, signal: AbortSignal) => Promise<Uint8Array>;
}

export function createResourcePdfRangeReader(location: ResourceLocation): PdfRangeReader {
  return { read: (offset, length, signal) => readResourceRange({ ...location, offset, length }, signal) };
}

export async function readPdfRangeInChunks(
  begin: number,
  end: number,
  reader: PdfRangeReader,
  signal: AbortSignal,
  onChunk?: (offset: number, chunk: Uint8Array) => void,
): Promise<void> {
  for (let offset = begin; offset < end; offset += PDF_RANGE_CHUNK_SIZE) {
    if (signal.aborted) throw new DOMException("PDF read was cancelled", "AbortError");
    const length = Math.min(PDF_RANGE_CHUNK_SIZE, end - offset);
    const chunk = await reader.read(offset, length, signal);
    if (chunk.byteLength > length) throw new Error("PDF range reader returned more bytes than requested.");
    onChunk?.(offset, chunk);
  }
}

export interface PdfDataRangeTransportLike {
  onDataRange: (begin: number, chunk: Uint8Array) => void;
  abort: () => void;
  requestDataRange: (begin: number, end: number) => void;
}

export type PdfDataRangeTransportConstructor<T extends PdfDataRangeTransportLike> = new (
  length: number,
  initialData: Uint8Array | null,
  progressiveDone?: boolean,
) => T;

/** Adapts PDF.js' requested windows to bounded native 256 KiB reads. */
export function createPdfDataRangeTransport<T extends PdfDataRangeTransportLike>(
  Constructor: PdfDataRangeTransportConstructor<T>,
  length: number,
  reader: PdfRangeReader,
  signal: AbortSignal,
): T {
  const transport = new Constructor(length, null, false);
  const pending = new Map<string, Promise<void>>();
  transport.requestDataRange = (begin, end) => {
    const key = `${begin}:${end}`;
    if (pending.has(key)) return;
    const request = readPdfRangeInChunks(begin, end, reader, signal, (offset, chunk) => {
      if (!signal.aborted) transport.onDataRange(offset, chunk);
    }).finally(() => pending.delete(key));
    pending.set(key, request);
    void request.catch(() => {
      // PDF.js will reject the loading task when its transport is aborted;
      // never publish partial data after a failed or stale request.
      transport.abort();
    });
  };
  transport.abort = () => {
    // The owning loading task calls destroy(); this method is intentionally
    // side-effect free so a stale request cannot cancel a newer document.
  };
  return transport;
}
