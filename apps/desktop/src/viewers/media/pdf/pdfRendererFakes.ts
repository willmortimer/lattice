import type {
  PdfOpenOptions,
  PdfPageHandle,
  PdfRendererAdapter,
  PdfRendererCapabilities,
  PdfRendererSession,
  PdfSource,
} from "./pdfRenderer";

export interface FakePdfSessionOptions {
  pageCount?: number;
  find?: (query: string, signal: AbortSignal) => Promise<number[]>;
  getPage?: (pageNumber: number) => Promise<PdfPageHandle>;
}

/** Test-only adapter for lifecycle and host-controller tests without PDF.js. */
export class FakePdfRendererAdapter implements PdfRendererAdapter {
  readonly id = "fake-pdf";
  readonly capabilities: PdfRendererCapabilities = {
    rangeLoading: true,
    selectableText: true,
    find: true,
  };
  opens: PdfSource[] = [];
  sessions: FakePdfRendererSession[] = [];

  constructor(private readonly options: FakePdfSessionOptions = {}) {}

  async open(source: PdfSource, _options: PdfOpenOptions): Promise<PdfRendererSession> {
    this.opens.push(source);
    const session = new FakePdfRendererSession(this.options);
    this.sessions.push(session);
    return session;
  }
}

export class FakePdfRendererSession implements PdfRendererSession {
  readonly pageCount: number;
  disposeCalls = 0;
  private disposed = false;

  constructor(private readonly options: FakePdfSessionOptions = {}) {
    this.pageCount = options.pageCount ?? 1;
  }

  async getPage(pageNumber: number, signal: AbortSignal): Promise<PdfPageHandle> {
    if (signal.aborted) throw new DOMException("PDF operation was cancelled", "AbortError");
    if (pageNumber < 1 || pageNumber > this.pageCount) throw new Error("PDF page is out of bounds.");
    if (!this.options.getPage) throw new Error("No fake page factory was provided.");
    return this.options.getPage(pageNumber);
  }

  async find(query: string, signal: AbortSignal): Promise<number[]> {
    if (this.disposed) throw new Error("The PDF session has been disposed.");
    return this.options.find ? this.options.find(query, signal) : [];
  }

  async dispose(): Promise<void> {
    if (this.disposed) return;
    this.disposed = true;
    this.disposeCalls += 1;
  }
}
