/**
 * Renderer-neutral PDF contracts. The viewer owns interaction and
 * virtualization; adapters own parser, worker, and rendering-engine state.
 */
export type PdfRendererErrorCode = "encrypted" | "malformed" | "oversized" | "worker" | "missing";

export class PdfRendererError extends Error {
  constructor(
    readonly code: PdfRendererErrorCode,
    message: string,
  ) {
    super(message);
    this.name = "PdfRendererError";
  }
}

export interface PdfSource {
  readonly id: string;
  readonly byteLength: number;
  readonly inspection: {
    readonly revision: string;
    readonly byteLength: number;
    readonly isDirectory: boolean;
  };
  /** Reads a bounded byte range and honours the caller's cancellation. */
  readRange(offset: number, length: number, signal: AbortSignal): Promise<Uint8Array>;
}

export interface PdfRendererCapabilities {
  readonly rangeLoading: boolean;
  readonly selectableText: boolean;
  readonly find: boolean;
}

export interface PdfOpenOptions {
  readonly signal: AbortSignal;
  readonly onProgress?: (progress: number) => void;
}

export interface PdfPageSize {
  readonly width: number;
  readonly height: number;
}

export interface PdfPageRenderOptions {
  readonly canvas: HTMLCanvasElement;
  readonly textLayer: HTMLElement;
  readonly scale: number;
  readonly deviceScale: number;
  readonly signal: AbortSignal;
}

export interface PdfPageRenderTask {
  readonly promise: Promise<PdfPageSize>;
  cancel(): void;
}

export interface PdfPageHandle {
  readonly pageNumber: number;
  getSize(signal: AbortSignal): Promise<PdfPageSize>;
  render(options: PdfPageRenderOptions): PdfPageRenderTask;
  dispose(): void;
}

export interface PdfRendererSession {
  readonly pageCount: number;
  getPage(pageNumber: number, signal: AbortSignal): Promise<PdfPageHandle>;
  /** Optional because not every future engine will expose searchable text. */
  find?(query: string, signal: AbortSignal): Promise<number[]>;
  /** Safe to call more than once; completion includes engine and worker teardown. */
  dispose(): Promise<void>;
}

export interface PdfRendererAdapter {
  readonly id: string;
  readonly capabilities: PdfRendererCapabilities;
  open(source: PdfSource, options: PdfOpenOptions): Promise<PdfRendererSession>;
}

export interface PdfRendererProvider {
  readonly defaultAdapter: PdfRendererAdapter;
}

export function createPdfRendererProvider(defaultAdapter: PdfRendererAdapter): PdfRendererProvider {
  return { defaultAdapter };
}
