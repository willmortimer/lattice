/**
 * Pure helpers for ResourceTree multi-select (click / cmd|ctrl / shift-range).
 * Selection is resourceId-based over visible file rows; folders are not selectable.
 */

export type TreeSelectMode = "replace" | "toggle" | "range";

export interface TreeSelectionInput {
  previous: ReadonlySet<string>;
  /** Last plain or toggle click used as the shift-range anchor. */
  anchor: string | null;
  clicked: string;
  /** Visible file resourceIds in display order (flattened tree). */
  visibleResourceIds: readonly string[];
  mode: TreeSelectMode;
}

export interface TreeSelectionResult {
  selected: ReadonlySet<string>;
  /** Anchor for the next shift-range (updated on replace/toggle). */
  anchor: string | null;
}

function rangeBetween(
  visibleResourceIds: readonly string[],
  from: string,
  to: string,
): Set<string> {
  const start = visibleResourceIds.indexOf(from);
  const end = visibleResourceIds.indexOf(to);
  if (start < 0 || end < 0) return new Set([to]);
  const lo = Math.min(start, end);
  const hi = Math.max(start, end);
  return new Set(visibleResourceIds.slice(lo, hi + 1));
}

/** Compute the next selected resourceId set for a tree click. */
export function nextTreeSelection(input: TreeSelectionInput): TreeSelectionResult {
  const { previous, anchor, clicked, visibleResourceIds, mode } = input;

  switch (mode) {
    case "replace":
      return { selected: new Set([clicked]), anchor: clicked };
    case "toggle": {
      const next = new Set(previous);
      if (next.has(clicked)) next.delete(clicked);
      else next.add(clicked);
      return { selected: next, anchor: clicked };
    }
    case "range": {
      const from = anchor && visibleResourceIds.includes(anchor) ? anchor : clicked;
      return { selected: rangeBetween(visibleResourceIds, from, clicked), anchor };
    }
    default: {
      const _exhaustive: never = mode;
      return _exhaustive;
    }
  }
}

/** ResourceIds to move when dropping `draggedId`, honoring a multi-selection. */
export function resourceIdsForTreeDrag(
  draggedId: string,
  selectedResourceIds: ReadonlySet<string>,
): string[] {
  if (selectedResourceIds.has(draggedId) && selectedResourceIds.size > 1) {
    return [...selectedResourceIds];
  }
  return [draggedId];
}

export function pathsForTreeDrag(
  draggedPath: string,
  selectedPaths: ReadonlySet<string>,
): string[] {
  return resourceIdsForTreeDrag(draggedPath, selectedPaths);
}
