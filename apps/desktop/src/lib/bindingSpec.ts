/**
 * Shared BindingSpec contract (mirrors Rust `lattice_data::BindingSpec` /
 * `lattice_commands::BindingSpec`). Kebab-case `type` tags; camelCase fields.
 */

export type BindingSpec =
  | { type: "resource"; resource: string }
  | { type: "saved-view"; resource: string; view: string }
  | { type: "sqlite-query"; resource: string; sql: string; limit: number }
  | { type: "duckdb-query"; resources: string[]; sql: string; limit: number }
  | { type: "notebook-output"; resource: string; cellId: string }
  | { type: "task-output"; resource: string; name: string };

export type InterfaceComponentType =
  | "metric"
  | "chart"
  | "map"
  | "form"
  | "data-view";

export interface InterfaceParameter {
  type: string;
  default?: unknown;
}

export interface InterfaceLayout {
  columns: number;
}

export interface InterfaceComponent {
  id: string;
  type: InterfaceComponentType;
  span: number;
  title?: string;
  binding?: BindingSpec;
  form?: string;
  chart?: string;
}

/** Full interface definition including optional dashboard components. */
export interface InterfaceDef {
  name: string;
  views: string[];
  forms: string[];
  title?: string;
  description?: string;
  parameters?: Record<string, InterfaceParameter>;
  layout?: InterfaceLayout;
  components?: InterfaceComponent[];
}

export function isBindingSpec(value: unknown): value is BindingSpec {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const record = value as Record<string, unknown>;
  switch (record.type) {
    case "resource":
      return typeof record.resource === "string";
    case "saved-view":
      return typeof record.resource === "string" && typeof record.view === "string";
    case "sqlite-query":
      return (
        typeof record.resource === "string" &&
        typeof record.sql === "string" &&
        typeof record.limit === "number"
      );
    case "duckdb-query":
      return (
        Array.isArray(record.resources) &&
        record.resources.every((item) => typeof item === "string") &&
        typeof record.sql === "string" &&
        typeof record.limit === "number"
      );
    case "notebook-output":
      return typeof record.resource === "string" && typeof record.cellId === "string";
    case "task-output":
      return typeof record.resource === "string" && typeof record.name === "string";
    default:
      return false;
  }
}

export function interfaceHasDashboardComponents(
  iface: Pick<InterfaceDef, "components">,
): boolean {
  return Array.isArray(iface.components) && iface.components.length > 0;
}
