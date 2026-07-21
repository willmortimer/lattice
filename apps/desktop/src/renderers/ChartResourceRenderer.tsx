import { useEffect, useMemo, useRef, useState } from "react";
import type { TopLevelSpec } from "vega-lite";

import { VegaLiteChart } from "../components/VegaLiteChart";
import "../components/vegaLiteChart.css";
import { inBrowser } from "../demo";
import { queryResultToValues } from "../lib/arrowToVegaData";
import { parseChartSpecText } from "../lib/chartSpec";
import { isDatasetRequestAborted } from "../lib/datasetCancel";
import { queryDatasetArrow } from "../lib/datasetQuery";
import { bindValuesToChartSpec } from "../lib/vegaLiteChart";
import type { OpenResourceSession } from "../resourceSession";
import type { ResourceRendererProps } from "../resourceRendererRegistry";
import type { ResourceRendererContext } from "./RendererContext";

type TextSession = Extract<OpenResourceSession, { kind: "text" }>;

export function ChartResourceRenderer({
  context,
  session,
}: ResourceRendererProps<ResourceRendererContext, OpenResourceSession>) {
  const textSession = session.kind === "text" ? (session as TextSession) : null;
  const root = context.workspaceRoot;
  const [spec, setSpec] = useState<TopLevelSpec | null>(null);
  const [meta, setMeta] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const loadAbortRef = useRef<AbortController | null>(null);

  const parsed = useMemo(() => {
    if (!textSession) return null;
    try {
      return parseChartSpecText(textSession.content);
    } catch (err) {
      return { error: err instanceof Error ? err.message : String(err) };
    }
  }, [textSession]);

  useEffect(() => {
    if (!textSession || !parsed || "error" in parsed) {
      setSpec(null);
      setMeta(null);
      setError(parsed && "error" in parsed ? parsed.error : null);
      setBusy(false);
      loadAbortRef.current = null;
      return;
    }

    const { spec: baseSpec, binding } = parsed;
    if (!binding) {
      setSpec(baseSpec);
      setMeta("Inline Vega-Lite spec");
      setError(null);
      setBusy(false);
      loadAbortRef.current = null;
      return;
    }

    if (!root) {
      setSpec(null);
      setMeta(null);
      setError("Open a native workspace to load dataset-bound chart data.");
      setBusy(false);
      loadAbortRef.current = null;
      return;
    }

    const controller = new AbortController();
    loadAbortRef.current = controller;
    setBusy(true);
    setError(null);
    void queryDatasetArrow(
      root,
      binding.dataset,
      {
        sql: binding.sql,
        maxRows: binding.maxRows,
      },
      controller.signal,
    )
      .then((result) => {
        if (controller.signal.aborted) return;
        const values = queryResultToValues(result, binding.maxRows);
        if (values.length === 0) {
          setSpec(null);
          setMeta(null);
          setError("Dataset query returned no rows to chart.");
          return;
        }
        setSpec(bindValuesToChartSpec(baseSpec, values));
        setMeta(
          `Bound to ${binding.dataset} → ${values.length} row${values.length === 1 ? "" : "s"}${result.truncated ? " (truncated)" : ""}`,
        );
      })
      .catch((err: unknown) => {
        if (controller.signal.aborted || isDatasetRequestAborted(err)) return;
        setSpec(null);
        setMeta(null);
        setError(err instanceof Error ? err.message : String(err));
      })
      .finally(() => {
        if (loadAbortRef.current === controller) {
          loadAbortRef.current = null;
          setBusy(false);
        }
      });

    return () => {
      controller.abort();
      if (loadAbortRef.current === controller) {
        loadAbortRef.current = null;
      }
    };
  }, [parsed, root, textSession, context.reloadToken]);

  if (!textSession) return null;

  if (inBrowser) {
    return (
      <div className="placeholder">
        <p className="placeholder-copy">Vega-Lite chart</p>
        <p className="placeholder-sub">
          <code>{textSession.resource.path}</code>
        </p>
        <div className="diagnostics-card" role="status">
          <strong>Visualization unavailable in browser demo</strong>
          <span>
            Dataset-bound charts need DuckDB + Arrow IPC in the native desktop app. Open this
            workspace with <code>nxr desktop-dev</code> or the installed Lattice.app.
          </span>
        </div>
      </div>
    );
  }

  return (
    <div className="placeholder">
      <p className="placeholder-copy">Vega-Lite chart</p>
      <p className="placeholder-sub">
        <code>{textSession.resource.path}</code>
      </p>
      {busy ? (
        <div className="dataset-surface-busy">
          <p className="placeholder-sub">Loading dataset query…</p>
          <button
            type="button"
            className="dataset-cancel-button"
            onClick={() => loadAbortRef.current?.abort()}
          >
            Cancel
          </button>
        </div>
      ) : null}
      {error ? (
        <p className="placeholder-sub" role="alert">
          {error}
        </p>
      ) : null}
      {meta ? <p className="dataset-chart-meta">{meta}</p> : null}
      {spec ? (
        <div className="dataset-chart-panel">
          <VegaLiteChart spec={spec} />
        </div>
      ) : null}
    </div>
  );
}
