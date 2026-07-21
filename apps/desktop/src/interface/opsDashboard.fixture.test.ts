import { describe, expect, it } from "vitest";

import { DEMO_OPS_DASHBOARD } from "../data/interfaces";
import { interfaceHasDashboardComponents } from "../lib/bindingSpec";

describe("OpsDashboard fixture", () => {
  it("ships at least three live component types", () => {
    expect(interfaceHasDashboardComponents(DEMO_OPS_DASHBOARD)).toBe(true);
    const types = new Set(DEMO_OPS_DASHBOARD.components?.map((item) => item.type));
    expect(types.has("metric")).toBe(true);
    expect(types.has("chart")).toBe(true);
    expect(types.has("map")).toBe(true);
    expect(types.size).toBeGreaterThanOrEqual(3);
  });
});
