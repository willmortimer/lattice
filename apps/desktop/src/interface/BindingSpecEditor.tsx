import type { BindingSpec, InterfaceComponentType } from "../lib/bindingSpec";
import { isBindingSpec } from "../lib/bindingSpec";

export type BindableKind =
  | "saved-view"
  | "duckdb-query"
  | "sqlite-query"
  | "resource"
  | "static-text";

export interface BindingSpecEditorProps {
  componentType: InterfaceComponentType;
  binding: BindingSpec | undefined;
  /** Form name for `form` components (sibling of binding). */
  formName?: string;
  /** Optional static title/body when binding is absent (text placeholder). */
  staticText?: string;
  views?: readonly string[];
  forms?: readonly string[];
  disabled?: boolean;
  onBindingChange: (binding: BindingSpec | undefined) => void;
  onFormNameChange?: (formName: string) => void;
  onStaticTextChange?: (text: string) => void;
  onValidationError?: (message: string | null) => void;
}

const KINDS_BY_TYPE: Record<InterfaceComponentType, BindableKind[]> = {
  metric: ["sqlite-query", "duckdb-query"],
  chart: ["duckdb-query"],
  map: ["duckdb-query", "resource"],
  "data-view": ["saved-view"],
  form: ["resource"],
};

function emptyBinding(kind: BindableKind): BindingSpec | undefined {
  switch (kind) {
    case "saved-view":
      return { type: "saved-view", resource: ".", view: "" };
    case "duckdb-query":
      return {
        type: "duckdb-query",
        resources: [""],
        sql: "",
        limit: 100,
      };
    case "sqlite-query":
      return { type: "sqlite-query", resource: ".", sql: "", limit: 1 };
    case "resource":
      return { type: "resource", resource: "." };
    case "static-text":
      return undefined;
    default: {
      const _exhaustive: never = kind;
      return _exhaustive;
    }
  }
}

function kindFromBinding(
  binding: BindingSpec | undefined,
  componentType: InterfaceComponentType,
): BindableKind {
  if (!binding) {
    if (componentType === "form") return "resource";
    return KINDS_BY_TYPE[componentType][0] ?? "resource";
  }
  switch (binding.type) {
    case "saved-view":
      return "saved-view";
    case "duckdb-query":
      return "duckdb-query";
    case "sqlite-query":
      return "sqlite-query";
    case "resource":
      return "resource";
    case "notebook-output":
    case "task-output":
      return KINDS_BY_TYPE[componentType][0] ?? "resource";
    default: {
      const _exhaustive: never = binding;
      return _exhaustive;
    }
  }
}

/** Compact BindingSpec fields for interface builder tiles. */
export function BindingSpecEditor({
  componentType,
  binding,
  formName = "",
  staticText = "",
  views = [],
  forms = [],
  disabled = false,
  onBindingChange,
  onFormNameChange,
  onStaticTextChange,
  onValidationError,
}: BindingSpecEditorProps) {
  const kinds = KINDS_BY_TYPE[componentType];
  const kind = kindFromBinding(binding, componentType);

  const emitBinding = (next: BindingSpec | undefined) => {
    if (next && !isBindingSpec(next)) {
      onValidationError?.("Binding is incomplete or invalid");
      onBindingChange(next);
      return;
    }
    onValidationError?.(null);
    onBindingChange(next);
  };

  return (
    <div className="lt-binding-editor" aria-label="Component binding">
      <label className="lt-binding-editor__field">
        <span>Bind</span>
        <select
          value={kind}
          disabled={disabled}
          aria-label="Binding kind"
          onChange={(event) => {
            const nextKind = event.target.value as BindableKind;
            if (nextKind === "static-text") {
              onValidationError?.(null);
              onBindingChange(undefined);
              return;
            }
            emitBinding(emptyBinding(nextKind));
          }}
        >
          {kinds.map((item) => (
            <option key={item} value={item}>
              {item}
            </option>
          ))}
        </select>
      </label>

      {componentType === "form" ? (
        <label className="lt-binding-editor__field">
          <span>Form</span>
          {forms.length > 0 ? (
            <select
              value={formName}
              disabled={disabled}
              aria-label="Package form"
              onChange={(event) => onFormNameChange?.(event.target.value)}
            >
              <option value="">Select form…</option>
              {forms.map((name) => (
                <option key={name} value={name}>
                  {name}
                </option>
              ))}
            </select>
          ) : (
            <input
              type="text"
              value={formName}
              disabled={disabled}
              placeholder="ContactIntake"
              aria-label="Package form name"
              onChange={(event) => onFormNameChange?.(event.target.value)}
            />
          )}
        </label>
      ) : null}

      {binding?.type === "saved-view" ? (
        <>
          <label className="lt-binding-editor__field">
            <span>Resource</span>
            <input
              type="text"
              value={binding.resource}
              disabled={disabled}
              aria-label="Saved view resource"
              onChange={(event) =>
                emitBinding({ ...binding, resource: event.target.value })
              }
            />
          </label>
          <label className="lt-binding-editor__field">
            <span>View</span>
            {views.length > 0 ? (
              <select
                value={binding.view}
                disabled={disabled}
                aria-label="Saved view name"
                onChange={(event) =>
                  emitBinding({ ...binding, view: event.target.value })
                }
              >
                <option value="">Select view…</option>
                {views.map((name) => (
                  <option key={name} value={name}>
                    {name}
                  </option>
                ))}
              </select>
            ) : (
              <input
                type="text"
                value={binding.view}
                disabled={disabled}
                aria-label="Saved view name"
                onChange={(event) =>
                  emitBinding({ ...binding, view: event.target.value })
                }
              />
            )}
          </label>
        </>
      ) : null}

      {binding?.type === "resource" ? (
        <label className="lt-binding-editor__field">
          <span>Resource</span>
          <input
            type="text"
            value={binding.resource}
            disabled={disabled}
            aria-label="Resource path"
            onChange={(event) =>
              emitBinding({ ...binding, resource: event.target.value })
            }
          />
        </label>
      ) : null}

      {binding?.type === "sqlite-query" ? (
        <>
          <label className="lt-binding-editor__field">
            <span>Resource</span>
            <input
              type="text"
              value={binding.resource}
              disabled={disabled}
              aria-label="SQLite resource"
              onChange={(event) =>
                emitBinding({ ...binding, resource: event.target.value })
              }
            />
          </label>
          <label className="lt-binding-editor__field lt-binding-editor__field--wide">
            <span>SQL</span>
            <textarea
              rows={3}
              value={binding.sql}
              disabled={disabled}
              aria-label="SQLite query"
              onChange={(event) =>
                emitBinding({ ...binding, sql: event.target.value })
              }
            />
          </label>
          <label className="lt-binding-editor__field">
            <span>Limit</span>
            <input
              type="number"
              min={1}
              value={binding.limit}
              disabled={disabled}
              aria-label="SQLite query limit"
              onChange={(event) =>
                emitBinding({
                  ...binding,
                  limit: Math.max(1, Number(event.target.value) || 1),
                })
              }
            />
          </label>
        </>
      ) : null}

      {binding?.type === "duckdb-query" ? (
        <>
          <label className="lt-binding-editor__field">
            <span>Resources</span>
            <input
              type="text"
              value={binding.resources.join(", ")}
              disabled={disabled}
              placeholder="Data/Orders.dataset"
              aria-label="DuckDB resources"
              onChange={(event) =>
                emitBinding({
                  ...binding,
                  resources: event.target.value
                    .split(",")
                    .map((part) => part.trim())
                    .filter(Boolean),
                })
              }
            />
          </label>
          <label className="lt-binding-editor__field lt-binding-editor__field--wide">
            <span>SQL</span>
            <textarea
              rows={3}
              value={binding.sql}
              disabled={disabled}
              aria-label="DuckDB query"
              onChange={(event) =>
                emitBinding({ ...binding, sql: event.target.value })
              }
            />
          </label>
          <label className="lt-binding-editor__field">
            <span>Limit</span>
            <input
              type="number"
              min={1}
              value={binding.limit}
              disabled={disabled}
              aria-label="DuckDB query limit"
              onChange={(event) =>
                emitBinding({
                  ...binding,
                  limit: Math.max(1, Number(event.target.value) || 1),
                })
              }
            />
          </label>
        </>
      ) : null}

      {!binding && onStaticTextChange ? (
        <label className="lt-binding-editor__field lt-binding-editor__field--wide">
          <span>Text</span>
          <textarea
            rows={2}
            value={staticText}
            disabled={disabled}
            aria-label="Static text"
            onChange={(event) => onStaticTextChange(event.target.value)}
          />
        </label>
      ) : null}
    </div>
  );
}

/** Validate a component binding draft before persist. */
export function validateComponentBinding(options: {
  type: InterfaceComponentType;
  binding?: BindingSpec;
  form?: string;
  packageForms?: readonly string[];
}): string | null {
  const { type, binding, form, packageForms = [] } = options;
  if (type === "form") {
    if (!form?.trim() && packageForms.length === 0) {
      return "Form components require a form name";
    }
  }
  if (type === "data-view") {
    if (!binding || binding.type !== "saved-view" || !binding.view.trim()) {
      return "Data view components require a saved-view binding with a view name";
    }
  }
  if (type === "metric" && binding && !isBindingSpec(binding)) {
    return "Metric binding is invalid";
  }
  if (type === "chart") {
    if (!binding || binding.type !== "duckdb-query" || !binding.sql.trim()) {
      return "Chart components require a duckdb-query binding with SQL";
    }
  }
  if (binding && !isBindingSpec(binding)) {
    return "Binding is incomplete or invalid";
  }
  return null;
}
