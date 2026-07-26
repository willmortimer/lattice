import {
  inspectResource,
  readResourceRange,
  type ResourceInspection,
  type ResourceLocation,
} from "../../../lib/resourceRuntime";
import { assertEncodedLimit, MAX_PDF_ENCODED_BYTES } from "../mediaLimits";
import { PDF_RANGE_CHUNK_SIZE } from "../mediaLimits";
import type { PdfSource } from "./pdfRenderer";
import { PdfRendererError } from "./pdfRenderer";

export interface PdfSourceLoader {
  inspect(location: ResourceLocation, signal: AbortSignal): Promise<ResourceInspection>;
  read(
    location: ResourceLocation & { offset: number; length: number },
    signal: AbortSignal,
  ): Promise<Uint8Array>;
}

const nativePdfSourceLoader: PdfSourceLoader = {
  inspect: inspectResource,
  read: readResourceRange,
};

/** Opens a workspace PDF as a bounded, cancellable source for a renderer. */
export async function openWorkspacePdfSource(
  location: ResourceLocation,
  signal: AbortSignal,
  loader: PdfSourceLoader = nativePdfSourceLoader,
): Promise<PdfSource> {
  let inspection: ResourceInspection;
  try {
    inspection = await loader.inspect(location, signal);
  } catch (error: unknown) {
    if (signal.aborted) throw error;
    throw new PdfRendererError("missing", "The PDF could not be inspected in the workspace.");
  }
  assertEncodedLimit(inspection.size, MAX_PDF_ENCODED_BYTES, "PDF");
  if (inspection.isDirectory) {
    throw new PdfRendererError("missing", "A directory cannot be opened as a PDF.");
  }

  return {
    id: `${location.root}:${location.path}:${inspection.revision}`,
    byteLength: inspection.size,
    inspection: {
      revision: inspection.revision,
      byteLength: inspection.size,
      isDirectory: inspection.isDirectory,
    },
    async readRange(offset, length, readSignal) {
      if (!Number.isSafeInteger(offset) || !Number.isSafeInteger(length) || offset < 0 || length < 0) {
        throw new Error("PDF range requests must use non-negative integer offsets and lengths.");
      }
      if (length > PDF_RANGE_CHUNK_SIZE) {
        throw new Error(`PDF range requests are limited to ${PDF_RANGE_CHUNK_SIZE} bytes.`);
      }
      if (offset > inspection.size || offset + length > inspection.size) {
        throw new Error("PDF range request extends beyond the source file.");
      }
      const bytes = await loader.read({ ...location, offset, length }, readSignal);
      if (bytes.byteLength > length) throw new Error("PDF source returned more bytes than requested.");
      return bytes;
    },
  };
}
