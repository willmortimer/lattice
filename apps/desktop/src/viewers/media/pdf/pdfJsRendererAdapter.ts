import type {
  PDFDataRangeTransport,
  PDFDocumentLoadingTask,
  PDFDocumentProxy,
  PDFPageProxy,
  PDFWorker,
  RenderTask,
} from "pdfjs-dist";
import { createPdfDataRangeTransport } from "../pdfRangeTransport";
import type {
  PdfOpenOptions,
  PdfPageHandle,
  PdfPageRenderOptions,
  PdfPageRenderTask,
  PdfPageSize,
  PdfRendererAdapter,
  PdfRendererCapabilities,
  PdfRendererSession,
  PdfSource,
} from "./pdfRenderer";
import { PdfRendererError } from "./pdfRenderer";

type PdfJsModule = typeof import("pdfjs-dist");

/**
 * The only production adapter today. Keeping PDF.js contained here lets the
 * host viewer stay stable as PDFium/EmbedPDF candidates are evaluated later.
 */
export class PdfJsRendererAdapter implements PdfRendererAdapter {
  readonly id = "pdfjs";
  readonly capabilities: PdfRendererCapabilities = {
    rangeLoading: true,
    selectableText: true,
    find: true,
  };

  async open(source: PdfSource, options: PdfOpenOptions): Promise<PdfRendererSession> {
    let pdfjs: PdfJsModule;
    try {
      pdfjs = await import("pdfjs-dist");
    } catch {
      throw new PdfRendererError("worker", "The built-in PDF engine could not be loaded.");
    }
    if (options.signal.aborted) throw abortError();
    return openPdfJsSession(pdfjs, source, options);
  }
}

export function createPdfJsRendererAdapter(): PdfRendererAdapter {
  return new PdfJsRendererAdapter();
}

async function openPdfJsSession(
  pdfjs: PdfJsModule,
  source: PdfSource,
  options: PdfOpenOptions,
): Promise<PdfRendererSession> {
  let loadingTask: PDFDocumentLoadingTask | null = null;
  let worker: PDFWorker | null = null;
  let transport: PDFDataRangeTransport | null = null;
  let passwordRequested = false;
  const operationController = new AbortController();
  const abortOperation = () => {
    operationController.abort();
    void loadingTask?.destroy();
  };
  options.signal.addEventListener("abort", abortOperation, { once: true });

  try {
    let workerAsset: { default: string };
    try {
      workerAsset = await import("pdfjs-dist/build/pdf.worker.min.mjs?url");
    } catch {
      throw new PdfRendererError("worker", "The packaged PDF worker asset could not be loaded.");
    }
    if (options.signal.aborted) throw abortError();
    pdfjs.GlobalWorkerOptions.workerSrc = workerAsset.default;
    worker = new pdfjs.PDFWorker();
    transport = createPdfDataRangeTransport(
      pdfjs.PDFDataRangeTransport,
      source.byteLength,
      { read: source.readRange.bind(source) },
      operationController.signal,
      () => { void loadingTask?.destroy(); },
    );
    loadingTask = pdfjs.getDocument({
      range: transport,
      rangeChunkSize: 256 * 1024,
      disableStream: true,
      disableAutoFetch: true,
      stopAtErrors: false,
      worker,
    });
    loadingTask.onProgress = ({ loaded, total }: { loaded: number; total: number }) => {
      options.onProgress?.(total > 0 ? Math.min(100, Math.round((loaded / total) * 100)) : 0);
    };
    loadingTask.onPassword = () => {
      passwordRequested = true;
      void loadingTask?.destroy();
    };
    const document = await loadingTask.promise;
    if (options.signal.aborted) {
      const session = new PdfJsRendererSession(
        pdfjs,
        document,
        loadingTask,
        worker,
        transport,
        operationController,
        () => options.signal.removeEventListener("abort", abortOperation),
      );
      await session.dispose();
      throw abortError();
    }
    return new PdfJsRendererSession(
      pdfjs,
      document,
      loadingTask,
      worker,
      transport,
      operationController,
      () => options.signal.removeEventListener("abort", abortOperation),
    );
  } catch (error: unknown) {
    operationController.abort();
    options.signal.removeEventListener("abort", abortOperation);
    transport?.abort();
    if (loadingTask) await loadingTask.destroy().catch(() => undefined);
    worker?.destroy();
    if (passwordRequested) {
      throw new PdfRendererError("encrypted", "This PDF is encrypted and cannot be opened in the built-in viewer.");
    }
    throw normalizePdfJsError(error);
  }
}

class PdfJsRendererSession implements PdfRendererSession {
  readonly pageCount: number;
  private disposePromise: Promise<void> | null = null;

  constructor(
    private readonly pdfjs: PdfJsModule,
    private readonly document: PDFDocumentProxy,
    private readonly loadingTask: PDFDocumentLoadingTask,
    private readonly worker: PDFWorker,
    private readonly transport: PDFDataRangeTransport,
    private readonly operationController: AbortController,
    private readonly detachExternalAbort: () => void,
  ) {
    this.pageCount = document.numPages;
  }

  async getPage(pageNumber: number, signal: AbortSignal): Promise<PdfPageHandle> {
    if (signal.aborted) throw abortError();
    if (!Number.isInteger(pageNumber) || pageNumber < 1 || pageNumber > this.pageCount) {
      throw new PdfRendererError("missing", "The requested PDF page does not exist.");
    }
    const page = await this.document.getPage(pageNumber);
    if (signal.aborted) {
      page.cleanup();
      throw abortError();
    }
    return new PdfJsPageHandle(this.pdfjs, page, pageNumber);
  }

  async find(query: string, signal: AbortSignal): Promise<number[]> {
    const matches: number[] = [];
    for (let pageNumber = 1; pageNumber <= this.pageCount; pageNumber += 1) {
      if (signal.aborted) throw abortError();
      const page = await this.document.getPage(pageNumber);
      try {
        if (signal.aborted) throw abortError();
        const content = await page.getTextContent();
        const text = content.items
          .map((item) => ("str" in item ? item.str : ""))
          .join(" ")
          .toLocaleLowerCase();
        if (text.includes(query.toLocaleLowerCase())) matches.push(pageNumber);
      } finally {
        page.cleanup();
      }
    }
    return matches;
  }

  dispose(): Promise<void> {
    if (this.disposePromise) return this.disposePromise;
    this.disposePromise = (async () => {
      this.operationController.abort();
      this.detachExternalAbort();
      this.transport.abort();
      await this.loadingTask.destroy().catch(() => undefined);
      this.document.cleanup();
      this.worker.destroy();
    })();
    return this.disposePromise;
  }
}

class PdfJsPageHandle implements PdfPageHandle {
  private disposed = false;

  constructor(
    private readonly pdfjs: PdfJsModule,
    private readonly page: PDFPageProxy,
    readonly pageNumber: number,
  ) {}

  async getSize(signal: AbortSignal): Promise<PdfPageSize> {
    if (signal.aborted) throw abortError();
    this.assertActive();
    const viewport = this.page.getViewport({ scale: 1 });
    return { width: viewport.width, height: viewport.height };
  }

  render(options: PdfPageRenderOptions): PdfPageRenderTask {
    this.assertActive();
    let renderTask: RenderTask | null = null;
    let textLayer: InstanceType<PdfJsModule["TextLayer"]> | null = null;
    const cancelForAbort = () => {
      renderTask?.cancel();
      textLayer?.cancel();
    };
    options.signal.addEventListener("abort", cancelForAbort, { once: true });
    const promise = (async () => {
      if (options.signal.aborted) throw abortError();
      const viewport = this.page.getViewport({ scale: options.scale });
      options.canvas.width = Math.max(1, Math.floor(viewport.width * options.deviceScale));
      options.canvas.height = Math.max(1, Math.floor(viewport.height * options.deviceScale));
      options.canvas.style.width = `${viewport.width}px`;
      options.canvas.style.height = `${viewport.height}px`;
      const canvasContext = options.canvas.getContext("2d");
      if (!canvasContext) throw new Error("This window cannot create a PDF canvas.");
      renderTask = this.page.render({
        canvasContext,
        canvas: options.canvas,
        viewport,
        transform: options.deviceScale === 1 ? undefined : [options.deviceScale, 0, 0, options.deviceScale, 0, 0],
      });
      await renderTask.promise;
      if (options.signal.aborted) throw abortError();
      this.assertActive();
      const textContent = await this.page.getTextContent();
      if (options.signal.aborted) throw abortError();
      this.assertActive();
      options.textLayer.replaceChildren();
      textLayer = new this.pdfjs.TextLayer({ textContentSource: textContent, container: options.textLayer, viewport });
      await textLayer.render();
      return { width: viewport.width / options.scale, height: viewport.height / options.scale };
    })().finally(() => options.signal.removeEventListener("abort", cancelForAbort));
    return {
      promise,
      cancel() {
        renderTask?.cancel();
        textLayer?.cancel();
      },
    };
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.page.cleanup();
  }

  private assertActive(): void {
    if (this.disposed) throw new Error("The PDF page has been disposed.");
  }
}

function normalizePdfJsError(error: unknown): Error {
  if (error instanceof PdfRendererError) return error;
  const message = error instanceof Error ? error.message : String(error);
  const lower = message.toLocaleLowerCase();
  if (lower.includes("password") || lower.includes("encrypted")) {
    return new PdfRendererError("encrypted", "This PDF is encrypted and cannot be opened in the built-in viewer.");
  }
  if (lower.includes("worker") || lower.includes("fake worker")) {
    return new PdfRendererError("worker", "The PDF worker could not start. Open the file externally or rebuild the desktop bundle.");
  }
  if (lower.includes("abort") || lower.includes("missing") || lower.includes("range")) {
    return new PdfRendererError("missing", "The PDF could not be read from the workspace.");
  }
  return new PdfRendererError("malformed", "This PDF is malformed or unsupported by the built-in viewer.");
}

function abortError(): DOMException {
  return new DOMException("PDF operation was cancelled", "AbortError");
}
