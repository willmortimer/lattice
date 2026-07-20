import { useEffect, useMemo, useState } from "react";
import type { TopLevelSpec } from "vega-lite";

import { VegaLiteChart } from "../components/VegaLiteChart";
import "../components/vegaLiteChart.css";
import { KindMark } from "../KindMark";
import type { ArrowTransportDump } from "../lib/arrowIpc";
import { queryResultToValues } from "../lib/arrowToVegaData";
import { loadDatasetArrowDump } from "../lib/datasetQuery";
import { buildAutoBarChartSpec } from "../lib/vegaLiteChart";
import type { OpenResourceSession } from "../resourceSession";
import type { ResourceRendererProps } from "../resourceRendererRegistry";
import type { ResourceRendererContext } from "./RendererContext";

type DatasetPanel = "preview" | "chart";

export function DatasetResourceRenderer({
  context,
  session,
}: ResourceRendererProps<ResourceRendererContext, OpenResourceSession>) {
  const isDataset = session.kind === "dataset";
  const root = context.workspaceRoot;
  const path = isDataset ? session.resource.path : "";
  const [panel, setPanel] = useState<DatasetPanel>("preview");
  const [dump, setDump] = useState<ArrowTransportDump | null>(null);
  const [queryResult, setQueryResult] = useState<Awaited<ReturnType<typeof loadDatasetArrowDump>>["result"] | null>(null);
  const [summary, setSummary] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!isDataset || !root) {
      setDump(null);
      setQueryResult(null);
      setSummary(null);
      setError(null);
      return;
    }
    let cancelled = false;
    setBusy(true);
    setError(null);
    void loadDatasetArrowDump(root, path)
      .then(({ dump: nextDump, summary: nextSummary, result }) => {
        if (cancelled) return;
        setDump(nextDump);
        setQueryResult(result);
        setSummary(nextSummary);
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        setDump(null);
        setQueryResult(null);
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

  const chartSpec = useMemo<TopLevelSpec | null>(() => {
    if (!dump || !queryResult) return null;
    const values = queryResultToValues(queryResult);
    return buildAutoBarChartSpec(dump.schema, values);
  }, [dump, queryResult]);

  if (!isDataset) return null;

  return (
    <div className="placeholder">
      <span className="placeholder-mark">
        <KindMark kind="dataset" size={36} />
      </span>
      <p className="placeholder-copy">Dataset</p>
      <p className="placeholder-sub">
        <code>{path}</code>
      </p>
      <div className="dataset-panel-tabs" role="tablist" aria-label="Dataset panels">
        <button
          type="button"
          role="tab"
          aria-selected={panel === "preview"}
          className={panel === "preview" ? "dataset-panel-tab dataset-panel-tab-active" : "dataset-panel-tab"}
          onClick={() => setPanel("preview")}
        >
          Preview
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={panel === "chart"}
          className={panel === "chart" ? "dataset-panel-tab dataset-panel-tab-active" : "dataset-panel-tab"}
          onClick={() => setPanel("chart")}
        >
          Chart
        </button>
      </div>
      {!root ? (
        <p className="placeholder-sub">Open a native workspace to run DuckDB → Arrow IPC queries.</p>
      ) : busy ? (
        <p className="placeholder-sub">Running bounded query…</p>
      ) : error ? (
        <p className="placeholder-sub" role="alert">
          {error}
        </p>
      ) : panel === "preview" ? (
        summary ? (
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
        ) : null
      ) : chartSpec ? (
        <div className="dataset-chart-panel">
          <p className="dataset-chart-meta">
            Auto bar chart from <code>{summary}</code>
          </p>
          <VegaLiteChart spec={chartSpec} />
        </div>
      ) : (
        <p className="placeholder-sub">No chartable rows yet. Import facts into this dataset package.</p>
      )}
    </div>
  );
}
