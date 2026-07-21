import { lazy, Suspense, useEffect, useMemo, useState } from "react";
import type { TopLevelSpec } from "vega-lite";

import { PerspectiveDatasetViewer } from "../analytics/PerspectiveDatasetViewer";
import "../analytics/perspective.css";
import { VegaLiteChart } from "../components/VegaLiteChart";
import "../components/vegaLiteChart.css";
import { inBrowser } from "../demo";
import { KindMark } from "../KindMark";
import type { ArrowQueryResult, ArrowTransportDump } from "../lib/arrowIpc";
import { queryResultToValues } from "../lib/arrowToVegaData";
import {
  explainDataset,
  type ExplainDatasetResponse,
} from "../lib/datasetExplain";
import { loadDatasetArrowDump } from "../lib/datasetQuery";
import {
  formatDistinct,
  formatPercent,
  formatProfileSummary,
  profileDataset,
  type RelationProfile,
} from "../lib/datasetProfile";
import { detectLonLatColumns } from "../lib/geoColumns";
import { buildAutoBarChartSpec } from "../lib/vegaLiteChart";
import type { OpenResourceSession } from "../resourceSession";
import type { ResourceRendererProps } from "../resourceRendererRegistry";
import type { ResourceRendererContext } from "./RendererContext";

const MapLibreDatasetViewer = lazy(async () => {
  const mod = await import("../analytics/MapLibreDatasetViewer");
  return { default: mod.MapLibreDatasetViewer };
});

type DatasetPanel = "preview" | "chart" | "profile" | "plan" | "map";

const DATASET_PANELS = [
  ["preview", "Preview"],
  ["chart", "Chart"],
  ["profile", "Profile"],
  ["plan", "Plan"],
  ["map", "Map"],
] as const satisfies ReadonlyArray<readonly [DatasetPanel, string]>;

function panelBusyLabel(panel: DatasetPanel): string {
  switch (panel) {
    case "profile":
      return "Profiling relation…";
    case "plan":
      return "Explaining query…";
    case "preview":
    case "chart":
    case "map":
      return "Running bounded query…";
    default: {
      const _exhaustive: never = panel;
      return _exhaustive;
    }
  }
}

/**
 * Dataset surface: Preview (Perspective), Chart (Vega-Lite), Profile (DuckDB SUMMARIZE),
 * Plan (DuckDB EXPLAIN), Map (MapLibre lon/lat).
 */
export function DatasetResourceRenderer({
  context,
  session,
}: ResourceRendererProps<ResourceRendererContext, OpenResourceSession>) {
  const isDataset = session.kind === "dataset";
  const root = context.workspaceRoot;
  const path = isDataset ? session.resource.path : "";
  const [panel, setPanel] = useState<DatasetPanel>("preview");
  const [result, setResult] = useState<ArrowQueryResult | null>(null);
  const [dump, setDump] = useState<ArrowTransportDump | null>(null);
  const [summary, setSummary] = useState<string | null>(null);
  const [profile, setProfile] = useState<RelationProfile | null>(null);
  const [profileSummary, setProfileSummary] = useState<string | null>(null);
  const [explain, setExplain] = useState<ExplainDatasetResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [viewerFailed, setViewerFailed] = useState(false);
  const [viewerError, setViewerError] = useState<string | null>(null);

  useEffect(() => {
    if (!isDataset || !root) {
      setResult(null);
      setDump(null);
      setSummary(null);
      setProfile(null);
      setProfileSummary(null);
      setExplain(null);
      setError(null);
      setViewerFailed(false);
      setViewerError(null);
      return;
    }
    let cancelled = false;
    setBusy(true);
    setError(null);

    const load = async () => {
      try {
        if (panel === "profile") {
          const nextProfile = await profileDataset(root, path);
          if (cancelled) return;
          setProfile(nextProfile);
          setProfileSummary(formatProfileSummary(nextProfile));
          return;
        }
        if (panel === "plan") {
          const nextExplain = await explainDataset(root, path);
          if (cancelled) return;
          setExplain(nextExplain);
          return;
        }
        setViewerFailed(false);
        setViewerError(null);
        const {
          result: nextResult,
          dump: nextDump,
          summary: nextSummary,
        } = await loadDatasetArrowDump(root, path);
        if (cancelled) return;
        setResult(nextResult);
        setDump(nextDump);
        setSummary(nextSummary);
      } catch (err: unknown) {
        if (cancelled) return;
        setResult(null);
        setDump(null);
        setSummary(null);
        setProfile(null);
        setProfileSummary(null);
        setExplain(null);
        setError(err instanceof Error ? err.message : String(err));
      } finally {
        if (!cancelled) setBusy(false);
      }
    };

    void load();
    return () => {
      cancelled = true;
    };
  }, [isDataset, root, path, context.reloadToken, panel]);

  const chartSpec = useMemo<TopLevelSpec | null>(() => {
    if (!dump || !result) return null;
    const values = queryResultToValues(result);
    return buildAutoBarChartSpec(dump.schema, values);
  }, [dump, result]);

  const mapRows = useMemo(() => {
    if (!result) return [];
    return queryResultToValues(result);
  }, [result]);

  const mapColumnNames = useMemo(
    () => (dump ? dump.schema.map((field) => field.name) : []),
    [dump],
  );

  const mapColumns = useMemo(
    () => detectLonLatColumns(mapColumnNames),
    [mapColumnNames],
  );

  if (!isDataset) return null;

  if (inBrowser) {
    return (
      <div className="dataset-surface">
        <header className="dataset-surface-header">
          <span className="placeholder-mark" aria-hidden>
            <KindMark kind="dataset" size={28} />
          </span>
          <div>
            <p className="dataset-surface-title">Dataset</p>
            <p className="dataset-surface-path">
              <code>{path}</code>
            </p>
          </div>
        </header>
        <div className="dataset-surface-body">
          <div className="diagnostics-card" role="status">
            <strong>Visualization unavailable in browser demo</strong>
            <span>
              Perspective Preview, Vega-Lite Chart, DuckDB Profile, EXPLAIN Plan, and MapLibre Map
              need the native desktop app (DuckDB + Arrow IPC). Open this workspace with{" "}
              <code>nxr desktop-dev</code> or the installed Lattice.app.
            </span>
          </div>
        </div>
      </div>
    );
  }

  const showPerspective = Boolean(root && result && !viewerFailed && !busy && !error);
  const loadKey = `${path}:${context.reloadToken}`;

  return (
    <div className="dataset-surface">
      <header className="dataset-surface-header">
        <span className="placeholder-mark" aria-hidden>
          <KindMark kind="dataset" size={28} />
        </span>
        <div>
          <p className="dataset-surface-title">Dataset</p>
          <p className="dataset-surface-path">
            <code>{path}</code>
          </p>
        </div>
        {panel === "profile"
          ? profileSummary && <p className="dataset-surface-meta">{profileSummary}</p>
          : panel === "plan"
            ? null
            : summary && <p className="dataset-surface-meta">{summary}</p>}
      </header>

      <div className="dataset-panel-tabs" role="tablist" aria-label="Dataset panels">
        {DATASET_PANELS.map(([id, label]) => (
          <button
            key={id}
            type="button"
            role="tab"
            aria-selected={panel === id}
            className={
              panel === id ? "dataset-panel-tab dataset-panel-tab-active" : "dataset-panel-tab"
            }
            onClick={() => setPanel(id)}
            disabled={!root || busy}
          >
            {label}
          </button>
        ))}
      </div>

      <div className="dataset-surface-body">
        <div className="dataset-surface-main">
          {!root ? (
            <div className="dataset-surface-fallback">
              <p className="placeholder-sub">
                Open a native workspace to run DuckDB → Arrow IPC → Perspective.
              </p>
            </div>
          ) : busy ? (
            <div className="dataset-surface-fallback">
              <p className="placeholder-sub">{panelBusyLabel(panel)}</p>
            </div>
          ) : error ? (
            <div className="dataset-surface-fallback">
              <p className="dataset-surface-alert" role="alert">
                {error}
              </p>
            </div>
          ) : panel === "plan" ? (
            explain ? (
              <div className="dataset-plan-panel">
                <section className="dataset-plan-section">
                  <h3 className="dataset-plan-heading">SQL</h3>
                  <pre className="dataset-plan-pre">{explain.sql}</pre>
                </section>
                <section className="dataset-plan-section">
                  <h3 className="dataset-plan-heading">Plan</h3>
                  <pre className="dataset-plan-pre">{explain.plan}</pre>
                </section>
              </div>
            ) : null
          ) : panel === "profile" ? (
            profile ? (
              profile.columns.length > 0 ? (
                <div className="dataset-profile-panel" style={{ overflow: "auto" }}>
                  <table>
                    <thead>
                      <tr>
                        <th scope="col">Column</th>
                        <th scope="col">Type</th>
                        <th scope="col">Null %</th>
                        <th scope="col">Distinct</th>
                        <th scope="col">Min</th>
                        <th scope="col">Max</th>
                        <th scope="col">Q50</th>
                      </tr>
                    </thead>
                    <tbody>
                      {profile.columns.map((column) => (
                        <tr key={column.name}>
                          <th scope="row">{column.name}</th>
                          <td>{column.dataType}</td>
                          <td>{formatPercent(column.nullPercentage)}</td>
                          <td>{formatDistinct(column.approxDistinct)}</td>
                          <td>{column.min ?? "—"}</td>
                          <td>{column.max ?? "—"}</td>
                          <td>{column.q50 ?? "—"}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              ) : (
                <p className="placeholder-sub">No columns to profile.</p>
              )
            ) : null
          ) : panel === "chart" ? (
            chartSpec ? (
              <div className="dataset-chart-panel">
                <p className="dataset-chart-meta">
                  Auto bar chart from <code>{summary}</code>
                </p>
                <VegaLiteChart spec={chartSpec} />
              </div>
            ) : (
              <p className="placeholder-sub">
                No chartable rows yet. Import facts into this dataset package.
              </p>
            )
          ) : panel === "map" ? (
            dump && result ? (
              <div className="dataset-map-panel">
                <p className="dataset-map-meta">
                  {mapColumns
                    ? `Points from ${mapColumns.lon}/${mapColumns.lat} · ${mapRows.length} row${mapRows.length === 1 ? "" : "s"}`
                    : "No geo columns"}
                  {summary ? (
                    <>
                      {" "}
                      · <code>{summary}</code>
                    </>
                  ) : null}
                </p>
                <Suspense
                  fallback={
                    <p className="placeholder-sub" aria-live="polite">
                      Loading map…
                    </p>
                  }
                >
                  <MapLibreDatasetViewer
                    rows={mapRows}
                    columnNames={mapColumnNames}
                    loadKey={loadKey}
                    onError={(message) => {
                      setViewerFailed(true);
                      setViewerError(message);
                    }}
                  />
                </Suspense>
              </div>
            ) : null
          ) : showPerspective && result ? (
            <PerspectiveDatasetViewer
              ipcBytes={result.ipcBytes}
              loadKey={loadKey}
              onError={(message) => {
                setViewerFailed(true);
                setViewerError(message);
              }}
            />
          ) : dump ? (
            <DatasetArrowFallback dump={dump} viewerError={viewerError} />
          ) : null}
        </div>
      </div>
    </div>
  );
}

function DatasetArrowFallback({
  dump,
  viewerError,
}: {
  dump: ArrowTransportDump;
  viewerError: string | null;
}) {
  return (
    <div className="dataset-surface-fallback">
      {viewerError ? (
        <p className="dataset-surface-alert" role="alert">
          Perspective unavailable — showing schema preview. ({viewerError})
        </p>
      ) : (
        <p className="placeholder-sub">Schema preview (analytical grid not loaded)</p>
      )}
      <pre>
        {JSON.stringify(
          {
            schema: dump.schema,
            sampleRows: dump.sampleRows,
            ipcBytes: dump.ipcByteLength,
            rowCount: dump.rowCount,
            truncated: dump.truncated,
          },
          null,
          2,
        )}
      </pre>
    </div>
  );
}
