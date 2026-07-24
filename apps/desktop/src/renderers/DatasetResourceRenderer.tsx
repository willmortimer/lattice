import { lazy, Suspense, useEffect, useMemo, useRef, useState } from "react";
import type { TopLevelSpec } from "vega-lite";

import { PerspectiveDatasetViewer } from "../analytics/PerspectiveDatasetViewer";
import { VegaLiteChart } from "../components/VegaLiteChart";
import "../components/vegaLiteChart.css";
import "./datasetSurface.css";
import { inBrowser } from "../demo";
import { KindMark } from "../KindMark";
import type { ArrowQueryResult, ArrowTransportDump } from "../lib/arrowIpc";
import { queryResultToValues } from "../lib/arrowToVegaData";
import { isDatasetRequestAborted } from "../lib/datasetCancel";
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
import { readTextWindow } from "../lib/resourceRuntime";
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

/** DuckDB types whose Min/Max/Q50 read best right-aligned. */
function isNumericDataType(dataType: string): boolean {
  return /INT|DECIMAL|NUMERIC|FLOAT|DOUBLE|REAL/i.test(dataType);
}

/** Display title fallback: `Reports/Usage.dataset` → `Usage`. */
function datasetPathTitle(path: string): string {
  const base = path.split("/").pop() ?? path;
  return base.replace(/\.dataset$/i, "") || path;
}

/**
 * Best-effort `title:` line from a dataset.yaml manifest window.
 * Full YAML parsing is unnecessary for one scalar top-level key.
 */
function parseManifestTitle(content: string): string | null {
  for (const line of content.split("\n")) {
    const match = /^title:\s*(.+)\s*$/.exec(line);
    if (!match) continue;
    const raw = match[1]!.trim();
    const unquoted = /^(['"])(.*)\1$/.exec(raw);
    const title = (unquoted ? unquoted[2]! : raw).trim();
    return title.length > 0 ? title : null;
  }
  return null;
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
  const queryKey = `${path}:${context.reloadToken}`;

  const [panel, setPanel] = useState<DatasetPanel>("preview");
  // Tabular query result (Preview / Chart / Map), cached across tab switches
  // for the same path + reload token so switching tabs does not re-run DuckDB.
  const [result, setResult] = useState<ArrowQueryResult | null>(null);
  const [dump, setDump] = useState<ArrowTransportDump | null>(null);
  const [summary, setSummary] = useState<string | null>(null);
  const [resultKey, setResultKey] = useState<string | null>(null);
  const [profile, setProfile] = useState<RelationProfile | null>(null);
  const [profileSummary, setProfileSummary] = useState<string | null>(null);
  const [profileKey, setProfileKey] = useState<string | null>(null);
  const [explain, setExplain] = useState<ExplainDatasetResponse | null>(null);
  const [planKey, setPlanKey] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  // Per-viewer failure state — a Map failure must not downgrade Preview.
  const [previewFailed, setPreviewFailed] = useState(false);
  const [previewError, setPreviewError] = useState<string | null>(null);
  const [mapError, setMapError] = useState<string | null>(null);
  const [manifestTitle, setManifestTitle] = useState<string | null>(null);
  const loadAbortRef = useRef<AbortController | null>(null);

  // Drop caches when the dataset itself changes.
  useEffect(() => {
    setResult(null);
    setDump(null);
    setSummary(null);
    setResultKey(null);
    setProfile(null);
    setProfileSummary(null);
    setProfileKey(null);
    setExplain(null);
    setPlanKey(null);
    setError(null);
    setPreviewFailed(false);
    setPreviewError(null);
    setMapError(null);
  }, [path]);

  // Best-effort manifest title (dataset.yaml lives inside the package).
  useEffect(() => {
    if (!isDataset || !root) {
      setManifestTitle(null);
      return;
    }
    const controller = new AbortController();
    void (async () => {
      try {
        const window = await readTextWindow(
          { root, path: `${path}/dataset.yaml`, offset: 0, length: 8192 },
          controller.signal,
        );
        if (controller.signal.aborted) return;
        setManifestTitle(parseManifestTitle(window.content));
      } catch {
        if (!controller.signal.aborted) setManifestTitle(null);
      }
    })();
    return () => controller.abort();
  }, [isDataset, root, path, context.reloadToken]);

  useEffect(() => {
    if (!isDataset || !root) {
      setError(null);
      setBusy(false);
      loadAbortRef.current = null;
      return;
    }
    const cached =
      panel === "profile"
        ? profileKey === queryKey
        : panel === "plan"
          ? planKey === queryKey
          : resultKey === queryKey;
    if (cached) {
      // A failure on another panel must not mask this panel's cached data.
      setError(null);
      return;
    }

    const controller = new AbortController();
    loadAbortRef.current = controller;
    setBusy(true);
    setError(null);

    const load = async () => {
      try {
        if (panel === "profile") {
          const nextProfile = await profileDataset(root, path, {}, controller.signal);
          if (controller.signal.aborted) return;
          setProfile(nextProfile);
          setProfileSummary(formatProfileSummary(nextProfile));
          setProfileKey(queryKey);
          return;
        }
        if (panel === "plan") {
          const nextExplain = await explainDataset(root, path, {}, controller.signal);
          if (controller.signal.aborted) return;
          setExplain(nextExplain);
          setPlanKey(queryKey);
          return;
        }
        setPreviewFailed(false);
        setPreviewError(null);
        setMapError(null);
        const {
          result: nextResult,
          dump: nextDump,
          summary: nextSummary,
        } = await loadDatasetArrowDump(root, path, {}, controller.signal);
        if (controller.signal.aborted) return;
        setResult(nextResult);
        setDump(nextDump);
        setSummary(nextSummary);
        setResultKey(queryKey);
      } catch (err: unknown) {
        if (controller.signal.aborted || isDatasetRequestAborted(err)) return;
        if (panel === "profile") {
          setProfile(null);
          setProfileSummary(null);
          setProfileKey(null);
        } else if (panel === "plan") {
          setExplain(null);
          setPlanKey(null);
        } else {
          setResult(null);
          setDump(null);
          setSummary(null);
          setResultKey(null);
        }
        setError(err instanceof Error ? err.message : String(err));
      } finally {
        if (loadAbortRef.current === controller) {
          loadAbortRef.current = null;
          setBusy(false);
        }
      }
    };

    void load();
    return () => {
      controller.abort();
      if (loadAbortRef.current === controller) {
        loadAbortRef.current = null;
      }
    };
  }, [isDataset, root, path, queryKey, panel, resultKey, profileKey, planKey]);

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

  const title = manifestTitle ?? datasetPathTitle(path);

  if (inBrowser) {
    return (
      <div className="dataset-surface">
        <header className="dataset-surface-header">
          <span className="dataset-surface-mark" aria-hidden>
            <KindMark kind="dataset" size={20} />
          </span>
          <div className="dataset-surface-heading">
            <p className="dataset-surface-title">{title}</p>
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

  const showPerspective = Boolean(root && result && !previewFailed && !busy && !error);
  const loadKey = `${path}:${context.reloadToken}`;
  const headerMeta =
    panel === "profile" ? profileSummary : panel === "plan" ? null : summary;

  return (
    <div className="dataset-surface">
      <header className="dataset-surface-header">
        <span className="dataset-surface-mark" aria-hidden>
          <KindMark kind="dataset" size={20} />
        </span>
        <div className="dataset-surface-heading">
          <p className="dataset-surface-title">{title}</p>
          <p className="dataset-surface-path">
            <code>{path}</code>
          </p>
        </div>
        {headerMeta ? <p className="dataset-surface-meta">{headerMeta}</p> : null}
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
            disabled={!root}
          >
            {label}
          </button>
        ))}
      </div>

      <div className="dataset-surface-body">
        <div className="dataset-surface-main">
          {!root ? (
            <div className="dataset-surface-fallback">
              <p className="placeholder-sub dataset-surface-note">
                Open a native workspace to run DuckDB → Arrow IPC → Perspective.
              </p>
            </div>
          ) : busy ? (
            <div className="dataset-surface-fallback dataset-surface-busy">
              <p className="placeholder-sub dataset-surface-note">{panelBusyLabel(panel)}</p>
              <button
                type="button"
                className="dataset-cancel-button"
                onClick={() => loadAbortRef.current?.abort()}
              >
                Cancel
              </button>
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
                <div className="dataset-profile-panel">
                  <table className="dataset-profile-table">
                    <thead>
                      <tr>
                        <th scope="col">Column</th>
                        <th scope="col">Type</th>
                        <th scope="col" className="dataset-profile-cell-numeric">
                          Null %
                        </th>
                        <th scope="col" className="dataset-profile-cell-numeric">
                          Distinct
                        </th>
                        <th scope="col">Min</th>
                        <th scope="col">Max</th>
                        <th scope="col">Q50</th>
                      </tr>
                    </thead>
                    <tbody>
                      {profile.columns.map((column) => {
                        const valueClass = isNumericDataType(column.dataType)
                          ? "dataset-profile-cell-numeric"
                          : "dataset-profile-cell-value";
                        return (
                          <tr key={column.name}>
                            <th scope="row">{column.name}</th>
                            <td className="dataset-profile-cell-type">{column.dataType}</td>
                            <td className="dataset-profile-cell-numeric">
                              {formatPercent(column.nullPercentage)}
                            </td>
                            <td className="dataset-profile-cell-numeric">
                              {formatDistinct(column.approxDistinct)}
                            </td>
                            <td className={valueClass}>{column.min ?? "—"}</td>
                            <td className={valueClass}>{column.max ?? "—"}</td>
                            <td className={valueClass}>{column.q50 ?? "—"}</td>
                          </tr>
                        );
                      })}
                    </tbody>
                  </table>
                </div>
              ) : (
                <p className="placeholder-sub dataset-surface-note">No columns to profile.</p>
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
              <p className="placeholder-sub dataset-surface-note">
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
                {mapError ? (
                  <p className="dataset-surface-alert" role="alert">
                    Map failed: {mapError}
                  </p>
                ) : null}
                <Suspense
                  fallback={
                    <p className="placeholder-sub dataset-surface-note" aria-live="polite">
                      Loading map…
                    </p>
                  }
                >
                  <MapLibreDatasetViewer
                    rows={mapRows}
                    columnNames={mapColumnNames}
                    loadKey={loadKey}
                    onError={(message) => {
                      setMapError(message);
                    }}
                  />
                </Suspense>
              </div>
            ) : null
          ) : showPerspective && result ? (
            <PerspectiveDatasetViewer
              ipcBytes={result.ipcBytes}
              schema={result.schemaMeta.fields}
              sampleRows={result.sampleRows ?? []}
              rowCount={result.rowCount}
              truncated={result.truncated}
              loadKey={loadKey}
              showDiagnostics={context.settings.diagnostics.showRendererStats}
              onError={(message) => {
                setPreviewFailed(true);
                setPreviewError(message);
              }}
            />
          ) : dump ? (
            <DatasetArrowFallback dump={dump} viewerError={previewError} />
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
          Analytical grid unavailable — {viewerError}
        </p>
      ) : (
        <p className="placeholder-sub dataset-surface-note">
          Analytical grid not loaded. Raw transport diagnostics below.
        </p>
      )}
      <div className="dataset-diagnostic-panel">
        <p className="dataset-diagnostic-title">Arrow transport diagnostics</p>
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
    </div>
  );
}
