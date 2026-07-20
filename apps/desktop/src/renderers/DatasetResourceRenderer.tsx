import { useEffect, useState } from "react";
import { KindMark } from "../KindMark";
import { PerspectiveDatasetViewer } from "../analytics/PerspectiveDatasetViewer";
import "../analytics/perspective.css";
import type { ArrowQueryResult, ArrowTransportDump } from "../lib/arrowIpc";
import { loadDatasetArrowDump } from "../lib/datasetQuery";
import type { OpenResourceSession } from "../resourceSession";
import type { ResourceRendererProps } from "../resourceRendererRegistry";
import type { ResourceRendererContext } from "./RendererContext";

/**
 * Dataset resource surface: Arrow IPC query → Perspective analytical grid.
 *
 * Sibling panels (profiling P3-05, charts P3-07) should compose next to
 * `dataset-surface-main` rather than rewriting this shell.
 */
export function DatasetResourceRenderer({
  context,
  session,
}: ResourceRendererProps<ResourceRendererContext, OpenResourceSession>) {
  const isDataset = session.kind === "dataset";
  const root = context.workspaceRoot;
  const path = isDataset ? session.resource.path : "";
  const [result, setResult] = useState<ArrowQueryResult | null>(null);
  const [dump, setDump] = useState<ArrowTransportDump | null>(null);
  const [summary, setSummary] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [viewerFailed, setViewerFailed] = useState(false);
  const [viewerError, setViewerError] = useState<string | null>(null);

  useEffect(() => {
    if (!isDataset || !root) {
      setResult(null);
      setDump(null);
      setSummary(null);
      setError(null);
      setViewerFailed(false);
      setViewerError(null);
      return;
    }
    let cancelled = false;
    setBusy(true);
    setError(null);
    setViewerFailed(false);
    setViewerError(null);
    void loadDatasetArrowDump(root, path)
      .then(({ result: nextResult, dump: nextDump, summary: nextSummary }) => {
        if (cancelled) return;
        setResult(nextResult);
        setDump(nextDump);
        setSummary(nextSummary);
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        setResult(null);
        setDump(null);
        setSummary(null);
        setError(err instanceof Error ? err.message : String(err));
      })
      .finally(() => {
        if (!cancelled) setBusy(false);
      });
    return () => {
      cancelled = true;
    };
  }, [isDataset, root, path, context.reloadToken]);

  if (!isDataset) return null;

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
        {summary ? <p className="dataset-surface-meta">{summary}</p> : null}
      </header>

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
              <p className="placeholder-sub">Running bounded query…</p>
            </div>
          ) : error ? (
            <div className="dataset-surface-fallback">
              <p className="dataset-surface-alert" role="alert">
                {error}
              </p>
            </div>
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
        {/* P3-05 profiling / P3-07 charts: compose as siblings of dataset-surface-main */}
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
