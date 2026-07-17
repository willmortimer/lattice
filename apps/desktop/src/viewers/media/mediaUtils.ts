export const NATIVE_RANGE_CHUNK_SIZE = 256 * 1024;

export function clampZoom(value: number, min = 0.1, max = 8): number {
  return Math.min(max, Math.max(min, value));
}

/** Select visible pages plus one-page overscan, bounded by the canvas budget. */
export function selectPdfRenderPages(
  pageCount: number,
  visiblePages: readonly number[],
  maxCanvases = 3,
): number[] {
  if (pageCount <= 0 || maxCanvases <= 0) return [];
  const visible = [...new Set(visiblePages)].filter((page) => page >= 1 && page <= pageCount);
  if (visible.length === 0) return [];
  const candidates = new Set<number>(visible);
  for (const page of visible) {
    if (page > 1) candidates.add(page - 1);
    if (page < pageCount) candidates.add(page + 1);
  }
  if (candidates.size <= maxCanvases) return [...candidates].sort((a, b) => a - b);

  const center = visible.reduce((sum, page) => sum + page, 0) / visible.length;
  const visibleSet = new Set(visible);
  return [...candidates]
    .sort((left, right) => {
      const visiblePriority = Number(visibleSet.has(left)) - Number(visibleSet.has(right));
      return visiblePriority || Math.abs(left - center) - Math.abs(right - center) || left - right;
    })
    .slice(0, maxCanvases)
    .sort((a, b) => a - b);
}

export function calculateVisiblePdfPages(
  pageCount: number,
  scrollTop: number,
  viewportHeight: number,
  scale: number,
  pageSizes: ReadonlyMap<number, { width: number; height: number }>,
  defaultPageSize = { width: 612, height: 792 },
  gap = 16,
): number[] {
  if (pageCount <= 0 || viewportHeight <= 0) return [];
  const visible: number[] = [];
  const viewportBottom = scrollTop + viewportHeight;
  let top = 0;
  for (let page = 1; page <= pageCount; page += 1) {
    const size = pageSizes.get(page) ?? defaultPageSize;
    const height = Math.max(1, size.height * scale);
    const bottom = top + height;
    if (bottom >= scrollTop && top <= viewportBottom) visible.push(page);
    top = bottom + gap;
    if (top > viewportBottom && visible.length > 0) break;
  }
  return visible;
}
