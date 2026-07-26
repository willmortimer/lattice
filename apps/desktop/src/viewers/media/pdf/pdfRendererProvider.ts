import { createPdfJsRendererAdapter } from "./pdfJsRendererAdapter";
import { createPdfRendererProvider, type PdfRendererProvider } from "./pdfRenderer";

/** The sole production selection point for PDF renderers. */
export function createDefaultPdfRendererProvider(): PdfRendererProvider {
  return createPdfRendererProvider(createPdfJsRendererAdapter());
}

let defaultProvider: PdfRendererProvider | null = null;

export function getDefaultPdfRendererProvider(): PdfRendererProvider {
  defaultProvider ??= createDefaultPdfRendererProvider();
  return defaultProvider;
}
