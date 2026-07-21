import { describe, expect, it } from "vitest";

import { DEMO_OPS_DASHBOARD } from "../data/interfaces";
import { interfaceHasDashboardComponents } from "../lib/bindingSpec";
import { applyParametersToBinding, initialParameterValues } from "./parameterSubstitution";

describe("OpsDashboard fixture", () => {
  it("ships at least three live component types", () => {
    expect(interfaceHasDashboardComponents(DEMO_OPS_DASHBOARD)).toBe(true);
    const types = new Set(DEMO_OPS_DASHBOARD.components?.map((item) => item.type));
    expect(types.has("metric")).toBe(true);
    expect(types.has("chart")).toBe(true);
    expect(types.has("map")).toBe(true);
    expect(types.size).toBeGreaterThanOrEqual(3);
  });

  it("declares a region parameter and substitutes it into chart/map SQL", () => {
    expect(DEMO_OPS_DASHBOARD.parameters?.region).toEqual({
      type: "string",
      default: "all",
    });
    const values = initialParameterValues(DEMO_OPS_DASHBOARD.parameters);
    expect(values).toEqual({ region: "all" });

    const chart = DEMO_OPS_DASHBOARD.components?.find((item) => item.id === "revenue_chart");
    expect(chart?.binding?.type).toBe("duckdb-query");
    if (chart?.binding?.type !== "duckdb-query") {
      throw new Error("expected duckdb-query chart binding");
    }
    expect(chart.binding.sql).toContain("{{region}}");
    const filtered = applyParametersToBinding(chart.binding, { region: "West" });
    if (filtered.type !== "duckdb-query") {
      throw new Error("expected duckdb-query after substitution");
    }
    expect(filtered.sql).toContain("region = 'West'");
    expect(filtered.sql).not.toContain("{{region}}");

    const map = DEMO_OPS_DASHBOARD.components?.find((item) => item.id === "places_map");
    expect(map?.binding?.type).toBe("duckdb-query");
    if (map?.binding?.type !== "duckdb-query") {
      throw new Error("expected duckdb-query map binding");
    }
    expect(map.binding.sql).toContain("{{region}}");
  });
});
