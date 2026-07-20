import { useEffect, useRef, useState } from "react";
import type { TopLevelSpec } from "vega-lite";

import "./vegaLiteChart.css";

export interface VegaLiteChartProps {
  spec: TopLevelSpec;
  className?: string;
}

let vegaThemeInitialized = false;

async function ensureVegaTheme(): Promise<void> {
  if (vegaThemeInitialized) return;
  const { default: vegaEmbed } = await import("vega-embed");
  vegaThemeInitialized = true;
  void vegaEmbed;
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
        await ensureVegaTheme();
        const { default: embed } = await import("vega-embed");
        if (cancelled) return;
        await embed(container, spec, {
          actions: false,
          renderer: "svg",
          theme: "dark",
        });
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
