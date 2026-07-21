import { describe, expect, it } from "vitest";

import {
  interfaceHasDashboardComponents,
  isBindingSpec,
  type BindingSpec,
} from "./bindingSpec";

describe("bindingSpec", () => {
  it("accepts all BindingSpec variants", () => {
    const cases: BindingSpec[] = [
      { type: "resource", resource: "CRM.data" },
      { type: "saved-view", resource: "CRM.data", view: "Board" },
      {
        type: "sqlite-query",
        resource: "CRM.data",
        sql: "SELECT COUNT(*) AS value FROM contacts",
        limit: 1,
      },
      {
        type: "duckdb-query",
        resources: ["Data/Orders.dataset"],
        sql: "SELECT 1 AS value",
        limit: 1,
      },
      {
        type: "notebook-output",
        resource: "Notebooks/Orders analytics.ipynb",
        cellId: "cell-1",
      },
      { type: "task-output", resource: "tasks/hello.task", name: "report" },
    ];
    for (const binding of cases) {
      expect(isBindingSpec(binding)).toBe(true);
    }
  });

  it("rejects malformed bindings", () => {
    expect(isBindingSpec({ type: "resource" })).toBe(false);
    expect(isBindingSpec({ type: "saved-view", resource: "x" })).toBe(false);
    expect(isBindingSpec({ type: "unknown", resource: "x" })).toBe(false);
  });

  it("detects dashboard components", () => {
    expect(interfaceHasDashboardComponents({})).toBe(false);
    expect(interfaceHasDashboardComponents({ components: [] })).toBe(false);
    expect(
      interfaceHasDashboardComponents({
        components: [{ id: "m", type: "metric", span: 3 }],
      }),
    ).toBe(true);
  });
});
