import { describe, expect, it } from "vitest";
import { retainedPdfPages, visiblePageRange, type PageMetric } from "./pdfVirtualization";

const pages: PageMetric[] = Array.from({ length: 8 }, () => ({ width: 600, height: 800 }));

describe("PDF page virtualization", () => {
  it("includes one page of overscan around the viewport", () => {
    expect(visiblePageRange(pages, 818, 800, 18, 1)).toEqual({ start: 0, end: 2 });
  });

  it("retains no more than three rendered pages", () => {
    expect(retainedPdfPages({ start: 1, end: 5 }, pages.length)).toHaveLength(3);
    expect(retainedPdfPages({ start: 1, end: 5 }, pages.length)).toEqual([2, 3, 4]);
  });
});
