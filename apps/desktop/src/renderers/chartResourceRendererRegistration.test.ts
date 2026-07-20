import { describe, expect, it } from "vitest";

import { ResourceRendererRegistry } from "../resourceRendererRegistry";
import type { OpenResourceSession } from "../resourceSession";
import { chartResourceRendererDefinition, registerChartResourceRenderers } from "./chartResourceRendererRegistration";
import type { ResourceRendererContext } from "./RendererContext";

describe("chartResourceRendererRegistration", () => {
  it("resolves .vl.json ahead of the generic JSON text viewer", () => {
    const fallback = {
      id: "fallback",
      kind: "*" as const,
      load: async () => () => null,
    };
    const registry = new ResourceRendererRegistry<ResourceRendererContext, OpenResourceSession>({
      capabilityFallback: fallback,
      unknownFallback: fallback,
    });
    registerChartResourceRenderers(registry);

    expect(
      registry.resolve({
        kind: "file",
        path: "Dashboards/Signups by region.vl.json",
        formatId: "file:vega-lite",
      }).definition.id,
    ).toBe(chartResourceRendererDefinition.id);
  });
});
