import type { BindingSpec } from "../lib/bindingSpec";

/** Resolve a binding resource path relative to the hosting `.data` package. */
export function resolveBindingResource(
  packagePath: string,
  resource: string | undefined,
): string {
  const trimmed = (resource ?? "").trim();
  if (!trimmed || trimmed === ".") {
    return packagePath.replace(/\\/g, "/");
  }
  return trimmed.replace(/\\/g, "/");
}

/** Primary dataset path for DuckDB bindings (first resource). */
export function primaryDuckdbResource(binding: Extract<BindingSpec, { type: "duckdb-query" }>): string {
  const first = binding.resources[0]?.trim();
  if (!first) {
    throw new Error("duckdb-query binding requires at least one resource");
  }
  return first.replace(/\\/g, "/");
}
