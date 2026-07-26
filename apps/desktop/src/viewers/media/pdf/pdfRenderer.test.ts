import { describe, expect, it } from "vitest";
import type {
  PdfOpenOptions,
  PdfPageHandle,
  PdfPageRenderOptions,
  PdfRendererAdapter,
  PdfRendererCapabilities,
  PdfRendererSession,
  PdfSource,
} from "./pdfRenderer";
import { createPdfRendererProvider } from "./pdfRenderer";
import { FakePdfRendererAdapter, FakePdfRendererSession } from "./pdfRendererFakes";

const source: PdfSource = {
  id: "test.pdf",
  byteLength: 1,
  inspection: { revision: "test", byteLength: 1, isDirectory: false },
  readRange: async () => new Uint8Array(),
};

describe("PDF renderer contracts", () => {
  it("uses an injectable default provider and disposes fake sessions exactly once", async () => {
    const adapter = new FakePdfRendererAdapter();
    const provider = createPdfRendererProvider(adapter);
    const session = await provider.defaultAdapter.open(source, { signal: new AbortController().signal });

    await Promise.all([session.dispose(), session.dispose(), session.dispose()]);

    expect(adapter.opens).toEqual([source]);
    expect(adapter.sessions[0].disposeCalls).toBe(1);
  });

  it("does not publish a stale session after its open signal is cancelled", async () => {
    const session = new FakePdfRendererSession();
    const adapter = new DelayedAdapter(session);
    const controller = new AbortController();
    const opening = adapter.open(source, { signal: controller.signal });
    controller.abort();
    adapter.resolve();

    await expect(opening).rejects.toMatchObject({ name: "AbortError" });
    expect(session.disposeCalls).toBe(1);
  });

  it("passes cancellation to page acquisition and render tasks", async () => {
    const controller = new AbortController();
    const page = new CancellablePage();
    const session = new FakePdfRendererSession({ getPage: async () => page });
    const handle = await session.getPage(1, controller.signal);
    const render = handle.render({
      canvas: {} as HTMLCanvasElement,
      textLayer: {} as HTMLElement,
      scale: 1,
      deviceScale: 1,
      signal: controller.signal,
    });
    controller.abort();

    expect(page.cancelCalls).toBe(1);
    void render.promise;
  });
});

class DelayedAdapter implements PdfRendererAdapter {
  readonly id = "delayed";
  readonly capabilities: PdfRendererCapabilities = { rangeLoading: true, selectableText: false, find: false };
  private release: (() => void) | null = null;

  constructor(private readonly session: PdfRendererSession) {}

  resolve(): void {
    this.release?.();
  }

  async open(_source: PdfSource, options: PdfOpenOptions): Promise<PdfRendererSession> {
    await new Promise<void>((resolve) => { this.release = resolve; });
    if (options.signal.aborted) {
      await this.session.dispose();
      throw new DOMException("PDF operation was cancelled", "AbortError");
    }
    return this.session;
  }
}

class CancellablePage implements PdfPageHandle {
  readonly pageNumber = 1;
  cancelCalls = 0;

  async getSize(_signal: AbortSignal) {
    return { width: 612, height: 792 };
  }

  render(options: PdfPageRenderOptions) {
    const cancel = () => { this.cancelCalls += 1; };
    options.signal.addEventListener("abort", cancel, { once: true });
    return { promise: new Promise<{ width: number; height: number }>(() => undefined), cancel };
  }

  dispose(): void {}
}
