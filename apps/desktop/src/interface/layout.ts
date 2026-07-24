import type {
  BindingSpec,
  InterfaceComponent,
  InterfaceComponentType,
} from "../lib/bindingSpec";

const DEFAULT_COLUMNS = 12;
const DEFAULT_INSERT_SPAN = 6;

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

/** Insert a component after `afterId`, or append when omitted / missing. */
export function insertComponent(
  components: readonly InterfaceComponent[],
  component: InterfaceComponent,
  afterId?: string | null,
): InterfaceComponent[] {
  if (components.some((item) => item.id === component.id)) {
    return [...components];
  }
  if (!afterId) return [...components, component];
  const index = components.findIndex((item) => item.id === afterId);
  if (index < 0) return [...components, component];
  const next = [...components];
  next.splice(index + 1, 0, component);
  return next;
}

/** Remove a component by id. */
export function removeComponent(
  components: readonly InterfaceComponent[],
  id: string,
): InterfaceComponent[] {
  return components.filter((item) => item.id !== id);
}

/** Patch one component by id (preserves unspecified fields). */
export function updateComponent(
  components: readonly InterfaceComponent[],
  id: string,
  patch: Partial<Omit<InterfaceComponent, "id">>,
): InterfaceComponent[] {
  return components.map((item) => (item.id === id ? { ...item, ...patch } : item));
}

/** Allocate a unique kebab-ish id from a type prefix. */
export function allocateComponentId(
  components: readonly InterfaceComponent[],
  type: InterfaceComponentType,
): string {
  const used = new Set(components.map((item) => item.id));
  const prefix = type.replace(/-/g, "_");
  if (!used.has(prefix)) return prefix;
  let n = 2;
  while (used.has(`${prefix}_${n}`)) n += 1;
  return `${prefix}_${n}`;
}

export interface CreateDefaultComponentOptions {
  columns?: number;
  views?: readonly string[];
  forms?: readonly string[];
  title?: string;
}

/** Sensible defaults for a newly inserted registry component. */
export function createDefaultComponent(
  type: InterfaceComponentType,
  components: readonly InterfaceComponent[],
  options: CreateDefaultComponentOptions = {},
): InterfaceComponent {
  const id = allocateComponentId(components, type);
  const span = clampSpan(DEFAULT_INSERT_SPAN, options.columns ?? DEFAULT_COLUMNS);
  const title = options.title ?? defaultTitleForType(type);
  const base: InterfaceComponent = { id, type, span, title };

  switch (type) {
    case "metric":
      return {
        ...base,
        span: clampSpan(3, options.columns ?? DEFAULT_COLUMNS),
        binding: defaultBindingForType(type),
      };
    case "chart":
    case "map":
      return { ...base, binding: defaultBindingForType(type) };
    case "data-view": {
      const view = options.views?.[0] ?? "Board";
      return {
        ...base,
        binding: { type: "saved-view", resource: ".", view },
      };
    }
    case "form": {
      const form = options.forms?.[0];
      return {
        ...base,
        form,
        binding: { type: "resource", resource: "." },
      };
    }
    default: {
      const _exhaustive: never = type;
      return _exhaustive;
    }
  }
}

function defaultTitleForType(type: InterfaceComponentType): string {
  switch (type) {
    case "metric":
      return "Metric";
    case "chart":
      return "Chart";
    case "map":
      return "Map";
    case "form":
      return "Form";
    case "data-view":
      return "Data view";
    default: {
      const _exhaustive: never = type;
      return _exhaustive;
    }
  }
}

/** Default BindingSpec seed for insert / bind editors. */
export function defaultBindingForType(type: InterfaceComponentType): BindingSpec | undefined {
  switch (type) {
    case "metric":
      return {
        type: "sqlite-query",
        resource: ".",
        sql: "SELECT COUNT(*) AS value FROM contacts",
        limit: 1,
      };
    case "chart":
    case "map":
      return {
        type: "duckdb-query",
        resources: ["Data/Orders.dataset"],
        sql: "SELECT 1 AS value",
        limit: 100,
      };
    case "data-view":
      return { type: "saved-view", resource: ".", view: "Board" };
    case "form":
      return { type: "resource", resource: "." };
    default: {
      const _exhaustive: never = type;
      return _exhaustive;
    }
  }
}
