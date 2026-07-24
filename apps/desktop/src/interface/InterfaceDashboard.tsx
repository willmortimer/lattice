import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";

import { openPackageSnapshot } from "../data/packageSnapshot";
import { inBrowser } from "../demo";
import { nativeOnlyDemoNotice } from "../data/browserDemoHonesty";
import type { DataAppSnapshot } from "../data/types";
import type {
  BindingSpec,
  InterfaceComponent,
  InterfaceComponentType,
  InterfaceDef,
} from "../lib/bindingSpec";
import { BindingSpecEditor, validateComponentBinding } from "./BindingSpecEditor";
import { INTERFACE_COMPONENT_TYPES, renderInterfaceComponent } from "./componentRegistry";
import {
  clampSpan,
  createDefaultComponent,
  insertComponent,
  layoutColumns,
  removeComponent,
  reorderComponents,
  resizeComponentSpan,
  updateComponent,
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
  /** Optional builder mode seed; defaults off. */
  initialBuilderMode?: boolean;
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
  initialBuilderMode = false,
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
  const [builderMode, setBuilderMode] = useState(initialBuilderMode);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [addType, setAddType] = useState<InterfaceComponentType>("metric");
  const [bindingError, setBindingError] = useState<string | null>(null);
  const [paramValues, setParamValues] = useState(() =>
    initialParameterValues(parameterDefs),
  );
  const [liveSnapshot, setLiveSnapshot] = useState<DataAppSnapshot | null>(snapshot);

  useEffect(() => {
    setLiveSnapshot(snapshot);
  }, [snapshot]);

  const builderAvailable = !readOnly;
  const builderNativeOnly = demo || inBrowser || !root;
  const editing = builderMode && builderAvailable && !builderNativeOnly;

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

  const refreshPackageSnapshot = useCallback(async () => {
    if (!root || demo) return;
    const fresh = await openPackageSnapshot(root, packagePath);
    setLiveSnapshot(fresh);
  }, [demo, packagePath, root]);

  useEffect(() => {
    if (selectedId && !components.some((item) => item.id === selectedId)) {
      setSelectedId(null);
    }
  }, [components, selectedId]);

  const host = useMemo(
    () => ({
      root,
      packagePath,
      demo,
      snapshot: liveSnapshot,
      packageRevision: liveSnapshot?.package_revision ?? null,
      paramValues,
      onOpenSavedView,
      onOpenResource,
      onPackageSnapshotRefresh: refreshPackageSnapshot,
    }),
    [
      demo,
      liveSnapshot,
      onOpenResource,
      onOpenSavedView,
      packagePath,
      paramValues,
      refreshPackageSnapshot,
      root,
    ],
  );

  const viewNames = useMemo(() => {
    const fromSnapshot = snapshot?.available_views ?? [];
    const fromDef = def.views ?? [];
    return Array.from(new Set([...fromSnapshot, ...fromDef]));
  }, [def.views, snapshot?.available_views]);

  const formNames = useMemo(() => {
    const fromDef = def.forms ?? [];
    return Array.from(new Set(fromDef));
  }, [def.forms]);

  const persist = useCallback(
    async (next: InterfaceDef) => {
      onDefChange?.(next);
      if (!root || demo || readOnly || inBrowser) return;
      setSaving(true);
      setPersistError(null);
      try {
        await savePackageInterface({
          root,
          relPath: packagePath,
          def: next,
        });
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

  const onAddComponent = useCallback(() => {
    if (!editing) return;
    const created = createDefaultComponent(addType, components, {
      columns,
      views: viewNames,
      forms: formNames,
    });
    const nextComponents = insertComponent(components, created, selectedId);
    setSelectedId(created.id);
    setBindingError(null);
    void persist({ ...def, components: nextComponents });
  }, [
    addType,
    columns,
    components,
    def,
    editing,
    formNames,
    persist,
    selectedId,
    viewNames,
  ]);

  const onRemoveSelected = useCallback(() => {
    if (!editing || !selectedId) return;
    const nextComponents = removeComponent(components, selectedId);
    setSelectedId(null);
    setBindingError(null);
    void persist({ ...def, components: nextComponents });
  }, [components, def, editing, persist, selectedId]);

  const onPatchSelected = useCallback(
    (patch: Partial<Omit<InterfaceComponent, "id">>) => {
      if (!editing || !selectedId) return;
      const current = components.find((item) => item.id === selectedId);
      if (!current) return;
      const merged = { ...current, ...patch };
      const validation = validateComponentBinding({
        type: merged.type,
        binding: merged.binding,
        form: merged.form,
        packageForms: formNames,
      });
      if (validation) {
        setBindingError(validation);
        // Keep local draft visible without destroying on-disk YAML until valid.
        onDefChange?.({
          ...def,
          components: updateComponent(components, selectedId, patch),
        });
        return;
      }
      setBindingError(null);
      void persist({
        ...def,
        components: updateComponent(components, selectedId, patch),
      });
    },
    [components, def, editing, formNames, onDefChange, persist, selectedId],
  );

  const selected = selectedId
    ? components.find((item) => item.id === selectedId) ?? null
    : null;

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
        <div className="lt-interface-dashboard__header-actions">
          {builderAvailable ? (
            <button
              type="button"
              className={`lt-interface-dashboard__builder-toggle${builderMode ? " is-active" : ""}`}
              aria-pressed={builderMode}
              onClick={() => setBuilderMode((prev) => !prev)}
            >
              {builderMode ? "Exit builder" : "Builder"}
            </button>
          ) : null}
          <p className="lt-interface-dashboard__meta" aria-live="polite">
            {saving ? "Saving…" : `${components.length} components · ${columns}-col grid`}
          </p>
        </div>
      </header>
      {builderMode && builderAvailable && builderNativeOnly ? (
        <p className="lt-interface-dashboard__notice" role="status">
          {nativeOnlyDemoNotice("Interface builder")}
        </p>
      ) : null}
      {editing ? (
        <div className="lt-interface-dashboard__builder" aria-label="Interface builder">
          <div className="lt-interface-dashboard__builder-row">
            <label className="lt-interface-dashboard__builder-field">
              <span>Add</span>
              <select
                value={addType}
                aria-label="Component type to add"
                onChange={(event) =>
                  setAddType(event.target.value as InterfaceComponentType)
                }
              >
                {INTERFACE_COMPONENT_TYPES.map((type) => (
                  <option key={type} value={type}>
                    {type}
                  </option>
                ))}
              </select>
            </label>
            <button type="button" onClick={onAddComponent}>
              Insert component
            </button>
            <button
              type="button"
              onClick={onRemoveSelected}
              disabled={!selectedId}
            >
              Remove selected
            </button>
          </div>
          {selected ? (
            <div className="lt-interface-dashboard__builder-editor">
              <p className="lt-interface-dashboard__builder-selected">
                Editing <code>{selected.id}</code> ({selected.type})
              </p>
              <label className="lt-interface-dashboard__builder-field">
                <span>Title</span>
                <input
                  type="text"
                  value={selected.title ?? ""}
                  aria-label={`Title for ${selected.id}`}
                  onChange={(event) =>
                    onPatchSelected({ title: event.target.value || undefined })
                  }
                />
              </label>
              <label className="lt-interface-dashboard__builder-field">
                <span>Span</span>
                <input
                  type="number"
                  min={1}
                  max={columns}
                  value={clampSpan(selected.span, columns)}
                  aria-label={`Span for ${selected.id}`}
                  onChange={(event) =>
                    onPatchSelected({
                      span: clampSpan(Number(event.target.value), columns),
                    })
                  }
                />
              </label>
              <BindingSpecEditor
                componentType={selected.type}
                binding={selected.binding}
                formName={selected.form ?? ""}
                views={viewNames}
                forms={formNames}
                onBindingChange={(binding: BindingSpec | undefined) =>
                  onPatchSelected({ binding })
                }
                onFormNameChange={(form) =>
                  onPatchSelected({ form: form || undefined })
                }
                onValidationError={setBindingError}
              />
            </div>
          ) : (
            <p className="lt-interface-dashboard__builder-hint">
              Select a tile to edit title, span, and binding.
            </p>
          )}
        </div>
      ) : null}
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
      {bindingError ? (
        <p className="lt-interface-dashboard__error" role="alert">
          {bindingError}
        </p>
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
            selected={selectedId === component.id}
            selectable={editing}
            dragging={dragId === component.id}
            onSelect={() => setSelectedId(component.id)}
            onDragStart={() => setDragId(component.id)}
            onDragEnd={() => setDragId(null)}
            onDrop={() => onDrop(component.id)}
            onResize={(span) => onResize(component.id, span)}
            onRemove={
              editing
                ? () => {
                    setSelectedId(component.id);
                    const nextComponents = removeComponent(components, component.id);
                    setSelectedId(null);
                    void persist({ ...def, components: nextComponents });
                  }
                : undefined
            }
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
  selected,
  selectable,
  dragging,
  onSelect,
  onDragStart,
  onDragEnd,
  onDrop,
  onResize,
  onRemove,
  children,
}: {
  component: InterfaceComponent;
  columns: number;
  readOnly: boolean;
  selected: boolean;
  selectable: boolean;
  dragging: boolean;
  onSelect: () => void;
  onDragStart: () => void;
  onDragEnd: () => void;
  onDrop: () => void;
  onResize: (span: number) => void;
  onRemove?: () => void;
  children: ReactNode;
}) {
  const span = clampSpan(component.span, columns);
  return (
    <div
      className={`lt-interface-tile${dragging ? " is-dragging" : ""}${selected ? " is-selected" : ""}`}
      style={{ gridColumn: `span ${span}` }}
      draggable={!readOnly}
      onClick={() => {
        if (selectable) onSelect();
      }}
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
          <span className="lt-interface-tile__type">{component.type}</span>
          <label className="lt-interface-tile__span">
            Span
            <input
              type="number"
              min={1}
              max={columns}
              value={span}
              aria-label={`Span for ${component.id}`}
              onClick={(event) => event.stopPropagation()}
              onChange={(event) => onResize(Number(event.target.value))}
            />
          </label>
          {onRemove ? (
            <button
              type="button"
              className="lt-interface-tile__remove"
              aria-label={`Remove ${component.id}`}
              onClick={(event) => {
                event.stopPropagation();
                onRemove();
              }}
            >
              Remove
            </button>
          ) : null}
        </div>
      ) : null}
      <div className="lt-interface-tile__body">{children}</div>
    </div>
  );
}
