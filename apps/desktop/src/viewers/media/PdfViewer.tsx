import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { FormEvent } from "react";
import type { Resource } from "../../types";
import type { ResourceRendererContext } from "../../renderers/RendererContext";
import { MediaDegraded } from "./MediaDegraded";
import { MediaLimitError } from "./mediaLimits";
import {
  calculateVisiblePdfPages,
  clampZoom,
  selectPdfRenderPages,
} from "./mediaUtils";
import { getDefaultPdfRendererProvider } from "./pdf/pdfRendererProvider";
import type {
  PdfPageHandle,
  PdfPageRenderTask,
  PdfRendererCapabilities,
  PdfRendererProvider,
  PdfRendererSession,
} from "./pdf/pdfRenderer";
import { PdfRendererError } from "./pdf/pdfRenderer";
import { openWorkspacePdfSource } from "./pdf/pdfSource";

export interface PdfViewerProps {
  context: ResourceRendererContext;
  resource: Resource;
  /** Injection point for adapter lifecycle and host-controller tests. */
  rendererProvider?: PdfRendererProvider;
}

type PdfState =
  | { status: "loading"; progress: number | null }
  | { status: "ready"; session: PdfRendererSession; capabilities: PdfRendererCapabilities }
  | { status: "error"; code: PdfRendererError["code"] | "oversized"; message: string };

interface PageSize {
  width: number;
  height: number;
}

interface PdfPageViewProps {
  session: PdfRendererSession;
  pageNumber: number;
  scale: number;
  onPageSize: (pageNumber: number, size: PageSize) => void;
  onError: (error: unknown) => void;
}

export function PdfViewer({ context, resource, rendererProvider = getDefaultPdfRendererProvider() }: PdfViewerProps) {
  const root = context.workspaceRoot;
  const path = resource.path;
  const viewportRef = useRef<HTMLDivElement>(null);
  const findControllerRef = useRef<AbortController | null>(null);
  const [state, setState] = useState<PdfState>({ status: "loading", progress: null });
  const [page, setPage] = useState(1);
  const [visiblePages, setVisiblePages] = useState([1]);
  const [pageSizes, setPageSizes] = useState<Map<number, PageSize>>(new Map());
  const [zoomMode, setZoomMode] = useState<"fit" | "actual" | "custom">("fit");
  const [customZoom, setCustomZoom] = useState(1);
  const [viewportSize, setViewportSize] = useState({ width: 0, height: 0 });
  const [findQuery, setFindQuery] = useState("");
  const [findStatus, setFindStatus] = useState<string | null>(null);

  useEffect(() => {
    const controller = new AbortController();
    let session: PdfRendererSession | null = null;
    setState({ status: "loading", progress: null });
    setPage(1);
    setVisiblePages([1]);
    setPageSizes(new Map());
    if (!root) {
      setState({ status: "error", code: "missing", message: "This PDF needs a native workspace reader." });
      return () => controller.abort();
    }

    void (async () => {
      try {
        const source = await openWorkspacePdfSource({ root, path }, controller.signal);
        if (controller.signal.aborted) return;
        session = await rendererProvider.defaultAdapter.open(source, {
          signal: controller.signal,
          onProgress: (progress) => {
          if (!controller.signal.aborted) setState({ status: "loading", progress });
          },
        });
        if (controller.signal.aborted) {
          await session.dispose();
          session = null;
          return;
        }
        setState({ status: "ready", session, capabilities: rendererProvider.defaultAdapter.capabilities });
      } catch (error: unknown) {
        if (controller.signal.aborted) return;
        setState(pdfErrorState(error));
      }
    })();

    return () => {
      controller.abort();
      findControllerRef.current?.abort();
      if (session) void session.dispose();
    };
  }, [path, rendererProvider, root]);

  useEffect(() => {
    const element = viewportRef.current;
    if (!element) return;
    const update = () => setViewportSize({ width: element.clientWidth, height: element.clientHeight });
    update();
    const observer = new ResizeObserver(update);
    observer.observe(element);
    return () => observer.disconnect();
  }, [state.status]);

  const basePageSize = pageSizes.get(1) ?? { width: 612, height: 792 };
  const fitScale = clampZoom(
    Math.min(viewportSize.width / basePageSize.width, viewportSize.height / basePageSize.height) || 1,
  );
  const scale = zoomMode === "fit" ? fitScale : zoomMode === "actual" ? 1 : customZoom;

  useEffect(() => {
    if (state.status !== "ready") return;
    const element = viewportRef.current;
    if (!element) return;
    let frame = 0;
    const update = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        const nextVisible = calculateVisiblePdfPages(
          state.session.pageCount,
          element.scrollTop,
          element.clientHeight,
          scale,
          pageSizes,
        );
        if (nextVisible.length > 0) {
          setVisiblePages(nextVisible);
          setPage(nextVisible[0]);
        }
      });
    };
    update();
    element.addEventListener("scroll", update, { passive: true });
    return () => {
      element.removeEventListener("scroll", update);
      cancelAnimationFrame(frame);
    };
  }, [pageSizes, scale, state]);

  const renderedPages = useMemo(
    () => (state.status === "ready" ? selectPdfRenderPages(state.session.pageCount, visiblePages) : []),
    [state, visiblePages],
  );

  const virtualPages = useMemo(() => {
    if (state.status !== "ready") return [];
    const pages = new Set<number>();
    for (const visiblePage of visiblePages) {
      pages.add(visiblePage);
      if (visiblePage > 1) pages.add(visiblePage - 1);
      if (visiblePage < state.session.pageCount) pages.add(visiblePage + 1);
    }
    return [...pages].sort((left, right) => left - right);
  }, [state, visiblePages]);

  function setZoom(next: number) {
    setCustomZoom(clampZoom(next));
    setZoomMode("custom");
  }

  function jumpToPage(nextPage: number) {
    if (state.status !== "ready") return;
    const next = Math.min(state.session.pageCount, Math.max(1, Math.round(nextPage)));
    setPage(next);
    const element = viewportRef.current;
    if (!element) return;
    let top = 0;
    for (let current = 1; current < next; current += 1) {
      const size = pageSizes.get(current) ?? { width: 612, height: 792 };
      top += size.height * scale + 16;
    }
    element.scrollTo({ top, behavior: "auto" });
  }

  async function findInDocument(event: FormEvent) {
    event.preventDefault();
    if (state.status !== "ready" || !state.capabilities.find || !state.session.find) return;
    const query = findQuery.trim().toLocaleLowerCase();
    if (!query) {
      setFindStatus(null);
      return;
    }
    findControllerRef.current?.abort();
    const controller = new AbortController();
    findControllerRef.current = controller;
    setFindStatus("Searching…");
    try {
      const matches = await state.session.find(query, controller.signal);
      if (matches.length > 0) {
        jumpToPage(matches[0]);
        setFindStatus(`${matches.length} page${matches.length === 1 ? "" : "s"} found`);
      } else {
        setFindStatus("No matches");
      }
    } catch (error: unknown) {
      if (!controller.signal.aborted) setFindStatus(`Find failed: ${errorMessage(error)}`);
    }
  }

  const onPageSize = useCallback((pageNumber: number, size: PageSize) => {
    setPageSizes((current) => {
      const previous = current.get(pageNumber);
      if (previous?.width === size.width && previous.height === size.height) return current;
      const next = new Map(current);
      next.set(pageNumber, size);
      return next;
    });
  }, []);

  const onPageError = useCallback((error: unknown) => {
    setState(pdfErrorState(error));
  }, []);

  if (state.status === "loading") {
    return <div className="media-viewer media-viewer-state">Loading PDF{state.progress === null ? "…" : ` · ${state.progress}%`}</div>;
  }
  if (state.status === "error") {
    return <MediaDegraded context={context} resource={resource} title="PDF preview unavailable" message={state.message} />;
  }

  return (
    <section className="media-viewer pdf-viewer" aria-label={`PDF viewer for ${path}`}>
      <header className="media-toolbar pdf-toolbar">
        <div className="media-toolbar-group">
          <button type="button" onClick={() => setZoomMode("fit")} aria-pressed={zoomMode === "fit"}>Fit</button>
          <button type="button" onClick={() => setZoomMode("actual")} aria-pressed={zoomMode === "actual"}>Actual</button>
          <button type="button" onClick={() => setZoom(scale - 0.1)} aria-label="Zoom out">−</button>
          <output className="media-zoom" aria-label="Zoom level">{Math.round(scale * 100)}%</output>
          <button type="button" onClick={() => setZoom(scale + 0.1)} aria-label="Zoom in">+</button>
        </div>
        <div className="pdf-page-controls">
          <button type="button" onClick={() => jumpToPage(page - 1)} disabled={page <= 1} aria-label="Previous page">‹</button>
          <label>
            <span className="sr-only">Page</span>
            <input
              type="number"
              min={1}
              max={state.session.pageCount}
              value={page}
              onChange={(event) => jumpToPage(Number(event.target.value))}
              aria-label="Current page"
            />
            <span> / {state.session.pageCount}</span>
          </label>
          <button type="button" onClick={() => jumpToPage(page + 1)} disabled={page >= state.session.pageCount} aria-label="Next page">›</button>
        </div>
        {state.capabilities.find && state.session.find && <form className="pdf-find" onSubmit={(event) => void findInDocument(event)}>
          <label>
            <span className="sr-only">Find in PDF</span>
            <input value={findQuery} onChange={(event) => setFindQuery(event.target.value)} placeholder="Find" />
          </label>
          <button type="submit">Find</button>
          {findStatus && <span className="pdf-find-status" aria-live="polite">{findStatus}</span>}
        </form>}
      </header>
      <div ref={viewportRef} className="pdf-pages-viewport">
        <div className="pdf-pages-stack" style={{ height: pageStackHeight(state.session.pageCount, scale, pageSizes) }}>
          {virtualPages.map((pageNumber) => {
            const size = pageSizes.get(pageNumber) ?? { width: 612, height: 792 };
            const isRendered = renderedPages.includes(pageNumber);
            return (
              <div
                className="pdf-page-slot"
                id={`pdf-page-${pageNumber}`}
                key={pageNumber}
                style={{ top: pageStackOffset(pageNumber, scale, pageSizes) + 18, width: size.width * scale, height: size.height * scale }}
              >
                {isRendered && (
                  <PdfPageView
                    session={state.session}
                    pageNumber={pageNumber}
                    scale={scale}
                    onPageSize={onPageSize}
                    onError={onPageError}
                  />
                )}
                {!isRendered && <div className="pdf-page-placeholder" aria-label={`Page ${pageNumber} is outside the render window`}>Page {pageNumber}</div>}
              </div>
            );
          })}
        </div>
      </div>
    </section>
  );
}

function pageStackOffset(pageNumber: number, scale: number, pageSizes: ReadonlyMap<number, PageSize>): number {
  let offset = 0;
  for (let current = 1; current < pageNumber; current += 1) {
    offset += (pageSizes.get(current)?.height ?? 792) * scale + 16;
  }
  return offset;
}

function pageStackHeight(pageCount: number, scale: number, pageSizes: ReadonlyMap<number, PageSize>): number {
  if (pageCount <= 0) return 0;
  return pageStackOffset(pageCount + 1, scale, pageSizes) + 40;
}

function PdfPageView({ session, pageNumber, scale, onPageSize, onError }: PdfPageViewProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const textLayerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const controller = new AbortController();
    let page: PdfPageHandle | null = null;
    let renderTask: PdfPageRenderTask | null = null;
    void (async () => {
      try {
        page = await session.getPage(pageNumber, controller.signal);
        if (controller.signal.aborted) return;
        const size = await page.getSize(controller.signal);
        if (controller.signal.aborted) return;
        onPageSize(pageNumber, size);
        const canvas = canvasRef.current;
        const textLayerElement = textLayerRef.current;
        if (!canvas || !textLayerElement) return;
        const deviceScale = Math.min(2, window.devicePixelRatio || 1);
        renderTask = page.render({
          canvas,
          textLayer: textLayerElement,
          scale,
          deviceScale,
          signal: controller.signal,
        });
        await renderTask.promise;
      } catch (error: unknown) {
        if (!controller.signal.aborted && !isRenderCancellation(error)) onError(error);
      }
    })();
    return () => {
      controller.abort();
      renderTask?.cancel();
      page?.dispose();
      textLayerRef.current?.replaceChildren();
    };
  }, [onError, onPageSize, pageNumber, scale, session]);

  return (
    <div className="pdf-page-view">
      <canvas ref={canvasRef} aria-label={`Rendered page ${pageNumber}`} />
      <div ref={textLayerRef} className="pdf-text-layer" aria-label={`Selectable text for page ${pageNumber}`} />
    </div>
  );
}

function pdfErrorState(error: unknown): Extract<PdfState, { status: "error" }> {
  if (error instanceof MediaLimitError) return { status: "error", code: "oversized", message: error.message };
  if (error instanceof PdfRendererError) return { status: "error", code: error.code, message: error.message };
  const message = errorMessage(error);
  const lower = message.toLocaleLowerCase();
  if (lower.includes("password") || lower.includes("encrypted")) {
    return { status: "error", code: "encrypted", message: "This PDF is encrypted and cannot be opened in the built-in viewer." };
  }
  if (lower.includes("worker") || lower.includes("fake worker")) {
    return { status: "error", code: "worker", message: "The PDF worker could not start. Open the file externally or rebuild the desktop bundle." };
  }
  if (lower.includes("abort") || lower.includes("missing") || lower.includes("range")) {
    return { status: "error", code: "missing", message: "The PDF could not be read from the workspace." };
  }
  return { status: "error", code: "malformed", message: "This PDF is malformed or unsupported by the built-in viewer." };
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function isRenderCancellation(error: unknown): boolean {
  return error instanceof Error && (error.name === "RenderingCancelledException" || error.name === "AbortException");
}

export type { PageSize };
