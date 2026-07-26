import { describe, expect, it } from "vitest";
import type { ResourceInspection, ResourceLocation } from "../../../lib/resourceRuntime";
import { PDF_RANGE_CHUNK_SIZE } from "../mediaLimits";
import { openWorkspacePdfSource, type PdfSourceLoader } from "./pdfSource";

const location: ResourceLocation = { root: "/workspace", path: "sources/paper.pdf" };

function inspection(size: number): ResourceInspection {
  return {
    path: location.path,
    kind: "file",
    profile: "pdf",
    capabilities: { canInspect: true, canReadRange: true, canReadTextWindow: false, canUpdate: false, isText: false, isBinary: true, validatesStructure: false, maxEditBytes: 0 },
    revision: "revision",
    size,
    isDirectory: false,
    probeBytes: 8,
    diagnostics: [],
  };
}

describe("workspace PDF source", () => {
  it("inspects once and only permits bounded contained reads", async () => {
    const reads: Array<{ offset: number; length: number }> = [];
    const loader: PdfSourceLoader = {
      inspect: async () => inspection(PDF_RANGE_CHUNK_SIZE + 1),
      read: async (range) => {
        reads.push({ offset: range.offset, length: range.length });
        return new Uint8Array(range.length);
      },
    };
    const source = await openWorkspacePdfSource(location, new AbortController().signal, loader);

    await source.readRange(4, 12, new AbortController().signal);
    await expect(source.readRange(0, PDF_RANGE_CHUNK_SIZE + 1, new AbortController().signal)).rejects.toThrow("limited");
    await expect(source.readRange(PDF_RANGE_CHUNK_SIZE, 2, new AbortController().signal)).rejects.toThrow("beyond");

    expect(source.id).toContain("revision");
    expect(source.inspection).toEqual({
      revision: "revision",
      byteLength: PDF_RANGE_CHUNK_SIZE + 1,
      isDirectory: false,
    });
    expect(reads).toEqual([{ offset: 4, length: 12 }]);
  });
});
