import { lazy, Suspense, useEffect, useState, type ReactNode } from "react";
import type { TopLevelSpec } from "vega-lite";

import { VegaLiteChart } from "../components/VegaLiteChart";
import "../components/vegaLiteChart.css";
import { inBrowser } from "../demo";
import { loadDataForm, type FormSummary } from "../data/forms";
import type { DataAppSnapshot } from "../data/types";
import { queryResultToValues } from "../lib/arrowToVegaData";
import type { BindingSpec, InterfaceComponent } from "../lib/bindingSpec";
import { isDatasetRequestAborted } from "../lib/datasetCancel";
import { queryDatasetArrow } from "../lib/datasetQuery";
import { buildAutoBarChartSpec } from "../lib/vegaLiteChart";
import { MetricCard } from "./MetricCard";
import { primaryDuckdbResource, resolveBindingResource } from "./resolveBinding";
import { queryDataSqlScalar } from "./saveInterface";

const MapLibreDatasetViewer = lazy(async () => {
  const mod = await import("../analytics/MapLibreDatasetViewer");
  return { default: mod.MapLibreDatasetViewer };
});

export interface InterfaceComponentHost {
  root: string | null;
  packagePath: string;
  demo?: boolean;
  /** Optional snapshot for same-package data-view / form embedding. */
  snapshot?: DataAppSnapshot | null;
  onOpenSavedView?: (viewName: string) => void;
  onOpenResource?: (path: string) => void;
}

export interface RenderInterfaceComponentProps {
  component: InterfaceComponent;
  host: InterfaceComponentHost;
}

function firstScalar(rows: Array<Record<string, unknown>>): string | number | null {
  const row = rows[0];
  if (!row) return null;
  const preferred = row.value ?? row.count ?? row.sum ?? Object.values(row)[0];
  if (typeof preferred === "number" || typeof preferred === "string") return preferred;
  if (typeof preferred === "bigint") return Number(preferred);
  if (preferred == null) return null;
  return String(preferred);
}

function MetricComponent({ component, host }: RenderInterfaceComponentProps) {
  const [value, setValue] = useState<string | number | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const binding = component.binding;

  useEffect(() => {
    if (!binding) {
      setError("Metric requires a binding");
      return;
    }
    if (!host.root || host.demo || inBrowser) {
      setValue(null);
      setError("Open a native workspace to load metric bindings.");
      return;
    }

    const controller = new AbortController();
    setBusy(true);
    setError(null);

    const run = async () => {
      if (binding.type === "sqlite-query") {
        const relPath = resolveBindingResource(host.packagePath, binding.resource);
        const result = await queryDataSqlScalar({
          root: host.root!,
          relPath,
          sql: binding.sql,
          limit: binding.limit,
        });
        if (controller.signal.aborted) return;
        setValue(result.value);
        return;
      }
      if (binding.type === "duckdb-query") {
        const dataset = primaryDuckdbResource(binding);
        const result = await queryDatasetArrow(
          host.root!,
          dataset,
          { sql: binding.sql, maxRows: binding.limit },
          controller.signal,
        );
        if (controller.signal.aborted) return;
        setValue(firstScalar(queryResultToValues(result, binding.limit)));
        return;
      }
      setError(`Unsupported metric binding type: ${binding.type}`);
    };

    void run()
      .catch((err: unknown) => {
        if (controller.signal.aborted || isDatasetRequestAborted(err)) return;
        setError(err instanceof Error ? err.message : String(err));
        setValue(null);
      })
      .finally(() => {
        if (!controller.signal.aborted) setBusy(false);
      });

    return () => controller.abort();
  }, [binding, host.demo, host.packagePath, host.root]);

  return (
    <MetricCard
      title={component.title ?? component.id}
      value={value}
      busy={busy}
      error={error}
      subtitle={binding?.type}
    />
  );
}

function ChartComponent({ component, host }: RenderInterfaceComponentProps) {
  const [spec, setSpec] = useState<TopLevelSpec | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const binding = component.binding;

  useEffect(() => {
    if (!binding || binding.type !== "duckdb-query") {
      setError("Chart components require a duckdb-query binding");
      return;
    }
    if (!host.root || host.demo || inBrowser) {
      setError("Open a native workspace to load chart data.");
      return;
    }
    const controller = new AbortController();
    setBusy(true);
    setError(null);
    const dataset = primaryDuckdbResource(binding);
    void queryDatasetArrow(
      host.root,
      dataset,
      { sql: binding.sql, maxRows: binding.limit },
      controller.signal,
    )
      .then((result) => {
        if (controller.signal.aborted) return;
        const values = queryResultToValues(result, binding.limit);
        if (values.length === 0) {
          setError("Chart query returned no rows");
          setSpec(null);
          return;
        }
        const auto = buildAutoBarChartSpec(result.schemaMeta.fields, values);
        if (!auto) {
          setError("Could not infer a chart encoding from the query result");
          setSpec(null);
          return;
        }
        setSpec(auto);
      })
      .catch((err: unknown) => {
        if (controller.signal.aborted || isDatasetRequestAborted(err)) return;
        setError(err instanceof Error ? err.message : String(err));
        setSpec(null);
      })
      .finally(() => {
        if (!controller.signal.aborted) setBusy(false);
      });
    return () => controller.abort();
  }, [binding, host.demo, host.root]);

  return (
    <div className="lt-interface-pane">
      <header className="lt-interface-pane__header">{component.title ?? component.id}</header>
      {busy ? <p className="lt-interface-pane__muted">Loading chart…</p> : null}
      {error ? (
        <p className="lt-interface-pane__error" role="alert">
          {error}
        </p>
      ) : null}
      {spec ? <VegaLiteChart spec={spec} /> : null}
    </div>
  );
}

function MapComponent({ component, host }: RenderInterfaceComponentProps) {
  const [rows, setRows] = useState<Array<Record<string, unknown>>>([]);
  const [columns, setColumns] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const binding = component.binding;

  useEffect(() => {
    const datasetPath =
      binding?.type === "resource"
        ? resolveBindingResource(host.packagePath, binding.resource)
        : binding?.type === "duckdb-query"
          ? primaryDuckdbResource(binding)
          : null;
    if (!datasetPath) {
      setError("Map components require a resource or duckdb-query binding");
      return;
    }
    if (!host.root || host.demo || inBrowser) {
      setError("Open a native workspace to load map data.");
      return;
    }
    const controller = new AbortController();
    setBusy(true);
    setError(null);
    const sql =
      binding?.type === "duckdb-query"
        ? binding.sql
        : undefined;
    void queryDatasetArrow(
      host.root,
      datasetPath,
      {
        sql,
        maxRows: binding?.type === "duckdb-query" ? binding.limit : 500,
      },
      controller.signal,
    )
      .then((result) => {
        if (controller.signal.aborted) return;
        const values = queryResultToValues(result, 500);
        setRows(values);
        setColumns(result.schemaMeta.fields.map((field) => field.name));
      })
      .catch((err: unknown) => {
        if (controller.signal.aborted || isDatasetRequestAborted(err)) return;
        setError(err instanceof Error ? err.message : String(err));
      })
      .finally(() => {
        if (!controller.signal.aborted) setBusy(false);
      });
    return () => controller.abort();
  }, [binding, host.demo, host.packagePath, host.root]);

  return (
    <div className="lt-interface-pane lt-interface-pane--map">
      <header className="lt-interface-pane__header">{component.title ?? component.id}</header>
      {busy ? <p className="lt-interface-pane__muted">Loading map…</p> : null}
      {error ? (
        <p className="lt-interface-pane__error" role="alert">
          {error}
        </p>
      ) : null}
      {!busy && !error ? (
        <Suspense fallback={<p className="lt-interface-pane__muted">Loading map viewer…</p>}>
          <MapLibreDatasetViewer rows={rows} columnNames={columns} />
        </Suspense>
      ) : null}
    </div>
  );
}

function DataViewComponent({ component, host }: RenderInterfaceComponentProps) {
  const binding = component.binding;
  const viewName =
    binding?.type === "saved-view"
      ? binding.view
      : host.snapshot?.active_view ?? component.title ?? "view";
  const title = component.title ?? viewName;

  return (
    <div className="lt-interface-pane">
      <header className="lt-interface-pane__header">{title}</header>
      <p className="lt-interface-pane__muted">
        Saved data view <code>{viewName}</code>
        {host.snapshot ? ` · ${host.snapshot.row_total} rows` : null}
      </p>
      {host.onOpenSavedView ? (
        <button
          type="button"
          className="lt-interface-pane__action"
          onClick={() => host.onOpenSavedView?.(viewName)}
        >
          Open full view
        </button>
      ) : null}
    </div>
  );
}

function FormComponent({ component, host }: RenderInterfaceComponentProps) {
  const formName = component.form;
  const [form, setForm] = useState<FormSummary | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!formName) {
      setError("Form component requires form:");
      return;
    }
    if (!host.root || host.demo || inBrowser) {
      setForm({
        name: formName,
        table: host.snapshot?.default_table ?? "contacts",
        fields: [],
        title: component.title,
      });
      return;
    }
    void loadDataForm(host.root, host.packagePath, formName)
      .then(setForm)
      .catch((err: unknown) => setError(err instanceof Error ? err.message : String(err)));
  }, [component.title, formName, host.demo, host.packagePath, host.root, host.snapshot?.default_table]);

  if (error) {
    return (
      <div className="lt-interface-pane">
        <header className="lt-interface-pane__header">{component.title ?? formName}</header>
        <p className="lt-interface-pane__error" role="alert">
          {error}
        </p>
      </div>
    );
  }
  if (!form) {
    return (
      <div className="lt-interface-pane">
        <p className="lt-interface-pane__muted">Loading form…</p>
      </div>
    );
  }

  return (
    <div className="lt-interface-pane">
      <header className="lt-interface-pane__header">{form.title ?? component.title ?? form.name}</header>
      <p className="lt-interface-pane__muted">
        Package form <code>{form.name}</code>
        {form.fields.length > 0 ? ` · fields: ${form.fields.join(", ")}` : null}
      </p>
      <p className="lt-interface-pane__muted">
        Use the data-app Forms panel to submit records for this package.
      </p>
    </div>
  );
}

function UnsupportedComponent({ component }: { component: InterfaceComponent }) {
  return (
    <div className="lt-interface-pane">
      <header className="lt-interface-pane__header">{component.title ?? component.id}</header>
      <p className="lt-interface-pane__muted">Unsupported component type.</p>
    </div>
  );
}

/** Registry entry: render one interface component by `type`. */
export function renderInterfaceComponent(
  component: InterfaceComponent,
  host: InterfaceComponentHost,
): ReactNode {
  switch (component.type) {
    case "metric":
      return <MetricComponent component={component} host={host} />;
    case "chart":
      return <ChartComponent component={component} host={host} />;
    case "map":
      return <MapComponent component={component} host={host} />;
    case "form":
      return <FormComponent component={component} host={host} />;
    case "data-view":
      return <DataViewComponent component={component} host={host} />;
    default: {
      const _exhaustive: never = component.type;
      void _exhaustive;
      return <UnsupportedComponent component={component} />;
    }
  }
}

/** Binding kinds used by the component registry (for tests / docs). */
export function bindingKindsForComponent(
  component: InterfaceComponent,
): BindingSpec["type"] | null {
  return component.binding?.type ?? null;
}
