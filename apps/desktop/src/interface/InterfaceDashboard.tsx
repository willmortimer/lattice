import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";

import type { DataAppSnapshot } from "../data/types";
import type { InterfaceComponent, InterfaceDef } from "../lib/bindingSpec";
import { renderInterfaceComponent } from "./componentRegistry";
import {
  clampSpan,
  layoutColumns,
  reorderComponents,
  resizeComponentSpan,
} from "./layout";
import { initialParameterValues } from "./parameterSubstitution";
import { savePackageInterface } from "./saveInterface";
import "./interfaceDashboard.css";

export interface InterfaceDashboardProps {
  root: string | null;
  packagePath: string;
  def: InterfaceDef;
  snapshot?: DataAppSnapshot | null;
  demo?: boolean;
  readOnly?: boolean;
  onDefChange?: (next: InterfaceDef) => void;
  onOpenSavedView?: (viewName: string) => void;
  onOpenResource?: (path: string) => void;
}

export function InterfaceDashboard({
  root,
  packagePath,
  def,
  snapshot = null,
  demo = false,
  readOnly = false,
  onDefChange,
  onOpenSavedView,
  onOpenResource,
}: InterfaceDashboardProps) {
  const columns = layoutColumns(def.layout);
  const components = def.components ?? [];
  const parameterDefs = def.parameters;
  const [dragId, setDragId] = useState<string | null>(null);
  const [persistError, setPersistError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [paramValues, setParamValues] = useState(() =>
    initialParameterValues(parameterDefs),
  );

  const parametersKey = useMemo(
    () =>
      JSON.stringify(
        Object.entries(parameterDefs ?? {}).map(([name, param]) => [
          name,
          param.type,
          param.default ?? null,
        ]),
      ),
    [parameterDefs],
  );

  useEffect(() => {
    setParamValues(initialParameterValues(parameterDefs));
    // parametersKey fingerprints declared defaults so a new object identity
    // with the same content does not wipe in-progress filter edits.
    // eslint-disable-next-line react-hooks/exhaustive-deps -- keyed by parametersKey
  }, [def.name, parametersKey]);

  const host = useMemo(
    () => ({
      root,
      packagePath,
      demo,
      snapshot,
      paramValues,
      onOpenSavedView,
      onOpenResource,
    }),
    [demo, onOpenResource, onOpenSavedView, packagePath, paramValues, root, snapshot],
  );

  const persist = useCallback(
    async (next: InterfaceDef) => {
      onDefChange?.(next);
      if (!root || demo || readOnly) return;
      setSaving(true);
      setPersistError(null);
      try {
        await savePackageInterface({ root, relPath: packagePath, def: next });
      } catch (error) {
        setPersistError(error instanceof Error ? error.message : String(error));
      } finally {
        setSaving(false);
      }
    },
    [demo, onDefChange, packagePath, readOnly, root],
  );

  const onDrop = useCallback(
    (targetId: string) => {
      if (!dragId || readOnly) return;
      const nextComponents = reorderComponents(components, dragId, targetId);
      setDragId(null);
      if (nextComponents === components) return;
      void persist({ ...def, components: nextComponents });
    },
    [components, def, dragId, persist, readOnly],
  );

  const onResize = useCallback(
    (id: string, span: number) => {
      if (readOnly) return;
      const nextComponents = resizeComponentSpan(components, id, span, columns);
      void persist({ ...def, components: nextComponents });
    },
    [columns, components, def, persist, readOnly],
  );

  const paramEntries = Object.entries(parameterDefs ?? {});

  return (
    <section className="lt-interface-dashboard" aria-label={def.title ?? def.name}>
      <header className="lt-interface-dashboard__header">
        <div>
          <h2 className="lt-interface-dashboard__title">{def.title ?? def.name}</h2>
          {def.description ? (
            <p className="lt-interface-dashboard__description">{def.description}</p>
          ) : null}
        </div>
        <p className="lt-interface-dashboard__meta" aria-live="polite">
          {saving ? "Saving…" : `${components.length} components · ${columns}-col grid`}
        </p>
      </header>
      {paramEntries.length > 0 ? (
        <form
          className="lt-interface-dashboard__filters"
          aria-label="Interface filters"
          onSubmit={(event) => event.preventDefault()}
        >
          {paramEntries.map(([name, param]) => (
            <label key={name} className="lt-interface-dashboard__filter">
              <span className="lt-interface-dashboard__filter-label">{name}</span>
              <input
                type="text"
                name={name}
                value={paramValues[name] ?? ""}
                placeholder={param.default == null ? undefined : String(param.default)}
                aria-label={`Filter ${name}`}
                onChange={(event) => {
                  const nextValue = event.target.value;
                  setParamValues((prev) => ({ ...prev, [name]: nextValue }));
                }}
              />
            </label>
          ))}
        </form>
      ) : null}
      {persistError ? (
        <p className="lt-interface-dashboard__error" role="alert">
          {persistError}
        </p>
      ) : null}
      <div
        className="lt-interface-dashboard__grid"
        style={{ gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))` }}
      >
        {components.map((component) => (
          <InterfaceTile
            key={component.id}
            component={component}
            columns={columns}
            readOnly={readOnly}
            dragging={dragId === component.id}
            onDragStart={() => setDragId(component.id)}
            onDragEnd={() => setDragId(null)}
            onDrop={() => onDrop(component.id)}
            onResize={(span) => onResize(component.id, span)}
          >
            {renderInterfaceComponent(component, host)}
          </InterfaceTile>
        ))}
      </div>
    </section>
  );
}

function InterfaceTile({
  component,
  columns,
  readOnly,
  dragging,
  onDragStart,
  onDragEnd,
  onDrop,
  onResize,
  children,
}: {
  component: InterfaceComponent;
  columns: number;
  readOnly: boolean;
  dragging: boolean;
  onDragStart: () => void;
  onDragEnd: () => void;
  onDrop: () => void;
  onResize: (span: number) => void;
  children: ReactNode;
}) {
  const span = clampSpan(component.span, columns);
  return (
    <div
      className={`lt-interface-tile${dragging ? " is-dragging" : ""}`}
      style={{ gridColumn: `span ${span}` }}
      draggable={!readOnly}
      onDragStart={(event) => {
        event.dataTransfer.effectAllowed = "move";
        onDragStart();
      }}
      onDragEnd={onDragEnd}
      onDragOver={(event) => {
        if (readOnly) return;
        event.preventDefault();
        event.dataTransfer.dropEffect = "move";
      }}
      onDrop={(event) => {
        event.preventDefault();
        onDrop();
      }}
    >
      {!readOnly ? (
        <div className="lt-interface-tile__chrome">
          <span className="lt-interface-tile__handle" title="Drag to reorder">
            ⋮⋮
          </span>
          <label className="lt-interface-tile__span">
            Span
            <input
              type="number"
              min={1}
              max={columns}
              value={span}
              aria-label={`Span for ${component.id}`}
              onChange={(event) => onResize(Number(event.target.value))}
            />
          </label>
        </div>
      ) : null}
      <div className="lt-interface-tile__body">{children}</div>
    </div>
  );
}
