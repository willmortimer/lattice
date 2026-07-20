import { useEffect, useState } from "react";
import { KindMark } from "../KindMark";
import type { ArrowTransportDump } from "../lib/arrowIpc";
import { loadDatasetArrowDump } from "../lib/datasetQuery";
import type { OpenResourceSession } from "../resourceSession";
import type { ResourceRendererProps } from "../resourceRendererRegistry";
import type { ResourceRendererContext } from "./RendererContext";

export function DatasetResourceRenderer({
  context,
  session,
}: ResourceRendererProps<ResourceRendererContext, OpenResourceSession>) {
  const isDataset = session.kind === "dataset";
  const root = context.workspaceRoot;
  const path = isDataset ? session.resource.path : "";
  const [dump, setDump] = useState<ArrowTransportDump | null>(null);
  const [summary, setSummary] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!isDataset || !root) {
      setDump(null);
      setSummary(null);
      setError(null);
      return;
    }
    let cancelled = false;
    setBusy(true);
    setError(null);
    void loadDatasetArrowDump(root, path)
      .then(({ dump: nextDump, summary: nextSummary }) => {
        if (cancelled) return;
        setDump(nextDump);
        setSummary(nextSummary);
      })
      .catch((err: unknown) => {
        if (cancelled) return;
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

  return (
    <div className="placeholder">
      <span className="placeholder-mark">
        <KindMark kind="dataset" size={36} />
      </span>
      <p className="placeholder-copy">Dataset (Arrow IPC preview)</p>
      <p className="placeholder-sub">
        <code>{path}</code>
      </p>
      {!root ? (
        <p className="placeholder-sub">Open a native workspace to run DuckDB → Arrow IPC queries.</p>
      ) : busy ? (
        <p className="placeholder-sub">Running bounded query…</p>
      ) : error ? (
        <p className="placeholder-sub" role="alert">
          {error}
        </p>
      ) : summary ? (
        <>
          <p className="placeholder-sub">{summary}</p>
          {dump && dump.sampleRows.length > 0 ? (
            <pre className="placeholder-sub" style={{ textAlign: "left", overflow: "auto" }}>
              {JSON.stringify(
                {
                  schema: dump.schema,
                  sampleRows: dump.sampleRows,
                  ipcBytes: dump.ipcByteLength,
                },
                null,
                2,
              )}
            </pre>
          ) : null}
        </>
      ) : null}
    </div>
  );
}
