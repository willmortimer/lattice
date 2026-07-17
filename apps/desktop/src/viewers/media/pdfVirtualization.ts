import { MAX_RENDERED_PDF_CANVASES } from "./mediaLimits";

export interface PageMetric {
  width: number;
  height: number;
}

export interface PageRange {
  start: number;
  end: number;
}

export function pageOffset(metrics: readonly PageMetric[], index: number, gap: number): number {
  let offset = 0;
  for (let current = 0; current < index; current += 1) offset += metrics[current].height + gap;
  return offset;
}

export function visiblePageRange(
  metrics: readonly PageMetric[],
  scrollTop: number,
  viewportHeight: number,
  gap: number,
  overscan = 1,
): PageRange {
  if (metrics.length === 0) return { start: 0, end: -1 };
  const bottom = scrollTop + Math.max(0, viewportHeight);
  let start = 0;
  let end = metrics.length - 1;
  let offset = 0;
  for (let index = 0; index < metrics.length; index += 1) {
    const next = offset + metrics[index].height;
    if (next >= scrollTop) {
      start = index;
      break;
    }
    offset = next + gap;
  }
  offset = 0;
  for (let index = 0; index < metrics.length; index += 1) {
    const next = offset + metrics[index].height;
    if (offset <= bottom && next >= bottom) {
      end = index;
      break;
    }
    offset = next + gap;
  }
  return {
    start: Math.max(0, start - overscan),
    end: Math.min(metrics.length - 1, end + overscan),
  };
}

export function retainedPdfPages(range: PageRange, pageCount: number, max = MAX_RENDERED_PDF_CANVASES): number[] {
  if (range.end < range.start || pageCount <= 0 || max <= 0) return [];
  const center = (range.start + range.end) / 2;
  const candidates = Array.from({ length: Math.max(0, range.end - range.start + 1) }, (_, index) => range.start + index)
    .filter((page) => page >= 0 && page < pageCount)
    .sort((left, right) => Math.abs(left - center) - Math.abs(right - center) || left - right);
  return candidates.slice(0, max).sort((left, right) => left - right);
}
