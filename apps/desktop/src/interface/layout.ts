import type { InterfaceComponent } from "../lib/bindingSpec";

const DEFAULT_COLUMNS = 12;

/** Clamp component span into the active column grid. */
export function clampSpan(span: number, columns: number = DEFAULT_COLUMNS): number {
  const cols = Math.max(1, Math.floor(columns));
  const raw = Number.isFinite(span) ? Math.floor(span) : 1;
  return Math.min(cols, Math.max(1, raw));
}

/** Reorder components by dragging `fromId` onto `toId`. */
export function reorderComponents(
  components: readonly InterfaceComponent[],
  fromId: string,
  toId: string,
): InterfaceComponent[] {
  if (fromId === toId) return [...components];
  const next = [...components];
  const fromIndex = next.findIndex((item) => item.id === fromId);
  const toIndex = next.findIndex((item) => item.id === toId);
  if (fromIndex < 0 || toIndex < 0) return next;
  const [moved] = next.splice(fromIndex, 1);
  if (!moved) return [...components];
  next.splice(toIndex, 0, moved);
  return next;
}

/** Resize one component's span (persisted as YAML `span`). */
export function resizeComponentSpan(
  components: readonly InterfaceComponent[],
  id: string,
  span: number,
  columns: number = DEFAULT_COLUMNS,
): InterfaceComponent[] {
  const nextSpan = clampSpan(span, columns);
  return components.map((item) => (item.id === id ? { ...item, span: nextSpan } : item));
}

export function layoutColumns(layout: { columns?: number } | undefined): number {
  const columns = layout?.columns;
  if (typeof columns === "number" && Number.isFinite(columns) && columns >= 1) {
    return Math.floor(columns);
  }
  return DEFAULT_COLUMNS;
}
