import type { BindingSpec, InterfaceParameter } from "../lib/bindingSpec";

/**
 * Interface dashboard parameter substitution for BindingSpec SQL.
 *
 * Placeholders use `{{name}}` where `name` is an identifier matching
 * `[A-Za-z_][A-Za-z0-9_]*` (same shape as package interface parameter keys).
 *
 * Replacement rules:
 * - Only exact `{{name}}` tokens are replaced; unknown names are left unchanged.
 * - Values are coerced with `String(...)`; `null`/`undefined` become `""`.
 * - Single quotes inside values are doubled (`'` → `''`) so templates can wrap
 *   placeholders in SQL string literals, e.g. `region = '{{region}}'`.
 * - The helper does **not** add quotes; authors must quote string params in SQL.
 * - Non-SQL bindings (`resource`, `saved-view`, …) are returned unchanged.
 *
 * Sentinel defaults such as `all` are a template concern, e.g.
 * `WHERE ('{{region}}' = 'all' OR region = '{{region}}')`.
 */

const PLACEHOLDER = /\{\{([A-Za-z_][A-Za-z0-9_]*)\}\}/g;

/** Escape a value for safe insertion into a SQL single-quoted string body. */
export function escapeSqlLiteral(value: string): string {
  return value.replace(/'/g, "''");
}

/** Replace `{{name}}` placeholders in a SQL string from parameter values. */
export function substituteParameters(
  sql: string,
  values: Record<string, string>,
): string {
  return sql.replace(PLACEHOLDER, (match, name: string) => {
    if (!Object.prototype.hasOwnProperty.call(values, name)) {
      return match;
    }
    return escapeSqlLiteral(values[name] ?? "");
  });
}

/** Initial filter-bar values from `InterfaceDef.parameters` defaults. */
export function initialParameterValues(
  parameters: Record<string, InterfaceParameter> | undefined,
): Record<string, string> {
  const out: Record<string, string> = {};
  if (!parameters) return out;
  for (const [name, param] of Object.entries(parameters)) {
    const raw = param.default;
    out[name] = raw == null ? "" : String(raw);
  }
  return out;
}

/**
 * Return a binding with SQL placeholders substituted for query kinds.
 * Non-query bindings are returned as-is (same reference).
 */
export function applyParametersToBinding(
  binding: BindingSpec,
  values: Record<string, string>,
): BindingSpec {
  switch (binding.type) {
    case "sqlite-query":
    case "duckdb-query":
      return { ...binding, sql: substituteParameters(binding.sql, values) };
    case "resource":
    case "saved-view":
    case "notebook-output":
    case "task-output":
      return binding;
    default: {
      const _exhaustive: never = binding;
      return _exhaustive;
    }
  }
}
