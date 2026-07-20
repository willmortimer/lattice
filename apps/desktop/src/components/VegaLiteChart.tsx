import { useEffect, useRef, useState } from "react";
import type { TopLevelSpec } from "vega-lite";

import "./vegaLiteChart.css";

export interface VegaLiteChartProps {
  spec: TopLevelSpec;
  className?: string;
}

/**
 * Packaged Tauri CSP omits `unsafe-eval`. Vega expression compilation uses
 * `new Function` by default; pass the CSP-safe interpreter via `ast: true`.
 */
async function embedChart(
  container: HTMLElement,
  spec: TopLevelSpec,
): Promise<void> {
  const [{ default: embed }, { expressionInterpreter }] = await Promise.all([
    import("vega-embed"),
    import("vega-interpreter"),
  ]);
  await embed(container, spec, {
    actions: false,
    renderer: "svg",
    theme: "dark",
    ast: true,
    expr: expressionInterpreter,
  });
}

/** Render a Vega-Lite spec with lazy-loaded vega-embed (chart panel only). */
export function VegaLiteChart({ spec, className }: VegaLiteChartProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    let cancelled = false;
    setError(null);
    container.replaceChildren();

    void (async () => {
      try {
        if (cancelled) return;
        await embedChart(container, spec);
      } catch (err) {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : String(err));
        }
      }
    })();

    return () => {
      cancelled = true;
      container.replaceChildren();
    };
  }, [spec]);

  return (
    <div className={className ? `vega-lite-chart ${className}` : "vega-lite-chart"}>
      <div ref={containerRef} className="vega-lite-chart-canvas" />
      {error ? (
        <p className="vega-lite-chart-error" role="alert">
          {error}
        </p>
      ) : null}
    </div>
  );
}
