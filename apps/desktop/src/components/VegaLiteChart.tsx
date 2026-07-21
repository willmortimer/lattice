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

function waitForLayoutWidth(el: HTMLElement, minWidth = 8): Promise<number> {
  if (el.clientWidth >= minWidth) return Promise.resolve(el.clientWidth);
  return new Promise((resolve) => {
    const observer = new ResizeObserver(() => {
      if (el.clientWidth >= minWidth) {
        observer.disconnect();
        resolve(el.clientWidth);
      }
    });
    observer.observe(el);
  });
}

/** Specs that use `width: "container"` need a real host width before embed. */
function withMeasuredWidth(spec: TopLevelSpec, width: number): TopLevelSpec {
  const record = spec as TopLevelSpec & { width?: unknown; height?: unknown };
  if (record.width === "container" || record.width === undefined) {
    return { ...spec, width: Math.max(280, Math.floor(width)) };
  }
  return spec;
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
        const width = await waitForLayoutWidth(container);
        if (cancelled) return;
        await embedChart(container, withMeasuredWidth(spec, width));
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
