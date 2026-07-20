import { useEffect, useMemo, useState } from "react";
import type { TopLevelSpec } from "vega-lite";

import { VegaLiteChart } from "../components/VegaLiteChart";
import "../components/vegaLiteChart.css";
import { queryResultToValues } from "../lib/arrowToVegaData";
import { parseChartSpecText } from "../lib/chartSpec";
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
      return;
    }

    const { spec: baseSpec, binding } = parsed;
    if (!binding) {
      setSpec(baseSpec);
      setMeta("Inline Vega-Lite spec");
      setError(null);
      return;
    }

    if (!root) {
      setSpec(null);
      setMeta(null);
      setError("Open a native workspace to load dataset-bound chart data.");
      return;
    }

    let cancelled = false;
    setBusy(true);
    setError(null);
    void queryDatasetArrow(root, binding.dataset, {
      sql: binding.sql,
      maxRows: binding.maxRows,
    })
      .then((result) => {
        if (cancelled) return;
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
        if (cancelled) return;
        setSpec(null);
        setMeta(null);
        setError(err instanceof Error ? err.message : String(err));
      })
      .finally(() => {
        if (!cancelled) setBusy(false);
      });

    return () => {
      cancelled = true;
    };
  }, [parsed, root, textSession, context.reloadToken]);

  if (!textSession) return null;

  return (
    <div className="placeholder">
      <p className="placeholder-copy">Vega-Lite chart</p>
      <p className="placeholder-sub">
        <code>{textSession.resource.path}</code>
      </p>
      {busy ? <p className="placeholder-sub">Loading dataset query…</p> : null}
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
