/**
 * Pure helpers for ResourceTree multi-select (click / cmd|ctrl / shift-range).
 * Selection is path-based over visible file rows; folders are not selectable.
 */

export type TreeSelectMode = "replace" | "toggle" | "range";

export interface TreeSelectionInput {
  previous: ReadonlySet<string>;
  /** Last plain or toggle click used as the shift-range anchor. */
  anchor: string | null;
  clicked: string;
  /** Visible file paths in display order (flattened tree). */
  visibleFilePaths: readonly string[];
  mode: TreeSelectMode;
}

export interface TreeSelectionResult {
  selected: ReadonlySet<string>;
  /** Anchor for the next shift-range (updated on replace/toggle). */
  anchor: string | null;
}

function rangeBetween(
  visibleFilePaths: readonly string[],
  from: string,
  to: string,
): Set<string> {
  const start = visibleFilePaths.indexOf(from);
  const end = visibleFilePaths.indexOf(to);
  if (start < 0 || end < 0) return new Set([to]);
  const lo = Math.min(start, end);
  const hi = Math.max(start, end);
  return new Set(visibleFilePaths.slice(lo, hi + 1));
}

/** Compute the next selected path set for a tree click. */
export function nextTreeSelection(input: TreeSelectionInput): TreeSelectionResult {
  const { previous, anchor, clicked, visibleFilePaths, mode } = input;

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
      const from = anchor && visibleFilePaths.includes(anchor) ? anchor : clicked;
      return { selected: rangeBetween(visibleFilePaths, from, clicked), anchor };
    }
    default: {
      const _exhaustive: never = mode;
      return _exhaustive;
    }
  }
}

/** Paths to move when dropping `draggedPath`, honoring a multi-selection. */
export function pathsForTreeDrag(
  draggedPath: string,
  selectedPaths: ReadonlySet<string>,
): string[] {
  if (selectedPaths.has(draggedPath) && selectedPaths.size > 1) {
    return [...selectedPaths];
  }
  return [draggedPath];
}
