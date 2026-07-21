import { describe, expect, it } from "vitest";

import type { BindingSpec } from "../lib/bindingSpec";
import {
  applyParametersToBinding,
  escapeSqlLiteral,
  initialParameterValues,
  substituteParameters,
} from "./parameterSubstitution";

describe("parameterSubstitution", () => {
  it("escapes single quotes for SQL literals", () => {
    expect(escapeSqlLiteral("O'Brien")).toBe("O''Brien");
  });

  it("replaces known {{name}} tokens and leaves unknowns", () => {
    const sql =
      "SELECT * FROM orders WHERE ('{{region}}' = 'all' OR region = '{{region}}') AND x = '{{missing}}'";
    expect(substituteParameters(sql, { region: "West" })).toBe(
      "SELECT * FROM orders WHERE ('West' = 'all' OR region = 'West') AND x = '{{missing}}'",
    );
  });

  it("escapes quotes inside substituted values", () => {
    expect(substituteParameters("name = '{{q}}'", { q: "a'b" })).toBe("name = 'a''b'");
  });

  it("builds initial values from parameter defaults", () => {
    expect(
      initialParameterValues({
        region: { type: "string", default: "all" },
        empty: { type: "string" },
      }),
    ).toEqual({ region: "all", empty: "" });
  });

  it("substitutes SQL on query bindings only", () => {
    const duck: BindingSpec = {
      type: "duckdb-query",
      resources: ["Data/Orders.dataset"],
      sql: "SELECT * FROM t WHERE region = '{{region}}'",
      limit: 10,
    };
    expect(applyParametersToBinding(duck, { region: "East" }).type).toBe("duckdb-query");
    expect(
      (applyParametersToBinding(duck, { region: "East" }) as Extract<
        BindingSpec,
        { type: "duckdb-query" }
      >).sql,
    ).toBe("SELECT * FROM t WHERE region = 'East'");

    const resource: BindingSpec = { type: "resource", resource: "Data/Places.dataset" };
    expect(applyParametersToBinding(resource, { region: "East" })).toBe(resource);
  });
});
