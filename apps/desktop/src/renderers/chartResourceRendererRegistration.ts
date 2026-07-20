import { ResourceRendererRegistry, type ResourceRendererDefinition } from "../resourceRendererRegistry";
import type { OpenResourceSession } from "../resourceSession";
import type { ResourceRendererContext } from "./RendererContext";

export type ChartResourceRendererDefinition = ResourceRendererDefinition<ResourceRendererContext, OpenResourceSession>;

function lazyChartRenderer(signal: AbortSignal): Promise<ChartResourceRendererDefinition["load"] extends (...args: never[]) => infer Result ? Awaited<Result> : never> {
  if (signal.aborted) return Promise.reject(new DOMException("Chart renderer load was cancelled", "AbortError"));
  return import("./ChartResourceRenderer").then((module) => {
    if (signal.aborted) throw new DOMException("Chart renderer load was cancelled", "AbortError");
    return module.ChartResourceRenderer;
  }) as ReturnType<typeof lazyChartRenderer>;
}

export const chartResourceRendererDefinition: ChartResourceRendererDefinition = {
  id: "vega-lite-chart",
  formatIds: ["vega-lite", "file:vega-lite"],
  surfaces: ["main"],
  load: lazyChartRenderer,
  lifecycle: { inactive: "unmount", cache: "module" },
};

export const chartResourceRendererDefinitions = [chartResourceRendererDefinition] as const;

/** Register Vega-Lite chart specs ahead of the generic JSON text viewer. */
export function registerChartResourceRenderers(
  registry: ResourceRendererRegistry<ResourceRendererContext, OpenResourceSession>,
): ResourceRendererRegistry<ResourceRendererContext, OpenResourceSession> {
  for (const definition of chartResourceRendererDefinitions) registry.register(definition);
  return registry;
}
