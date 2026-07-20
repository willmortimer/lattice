import { describe, expect, it } from "vitest";

import { sampleRowsToValues } from "./arrowToVegaData";
import { parseChartSpecDocument, parseChartSpecText } from "./chartSpec";
import { buildAutoBarChartSpec, bindValuesToChartSpec } from "./vegaLiteChart";

describe("chartSpec", () => {
  it("extracts lattice dataset bindings from a chart document", () => {
    const parsed = parseChartSpecDocument({
      lattice: {
        data: {
          dataset: "Analytics/Usage.dataset",
          sql: "SELECT category, total FROM facts",
          maxRows: 500,
        },
      },
      $schema: "https://vega.github.io/schema/vega-lite/v6.json",
      mark: "bar",
      encoding: {
        x: { field: "category", type: "nominal" },
        y: { field: "total", type: "quantitative" },
      },
    });

    expect(parsed.binding).toEqual({
      dataset: "Analytics/Usage.dataset",
      sql: "SELECT category, total FROM facts",
      maxRows: 500,
    });
    expect(parsed.spec).not.toHaveProperty("lattice");
    expect(parsed.spec.mark).toBe("bar");
  });

  it("parses chart JSON text", () => {
    const parsed = parseChartSpecText(
      JSON.stringify({
        lattice: { data: { dataset: "Usage.dataset" } },
        data: { name: "table" },
        mark: "point",
      }),
    );
    expect(parsed.binding?.dataset).toBe("Usage.dataset");
    expect(parsed.spec).toMatchObject({ mark: "point" });
  });
});

describe("vegaLiteChart", () => {
  const schema = [
    { name: "category", dataType: "utf8", nullable: true },
    { name: "total", dataType: "int64", nullable: false },
  ];
  const values = sampleRowsToValues(
    [
      ["North", 12],
      ["South", 8],
    ],
    schema,
  );

  it("builds an auto bar chart from schema + values", () => {
    const spec = buildAutoBarChartSpec(schema, values);
    expect(spec?.encoding?.x).toMatchObject({ field: "category", type: "nominal" });
    expect(spec?.encoding?.y).toMatchObject({ field: "total", type: "quantitative" });
    expect(spec?.data).toEqual({ values });
  });

  it("binds named datasets for vega-lite rendering", () => {
    const spec = bindValuesToChartSpec(
      {
        data: { name: "table" },
        mark: "bar",
        encoding: {
          x: { field: "category", type: "nominal" },
          y: { field: "total", type: "quantitative" },
        },
      },
      values,
    );
    expect(spec.datasets).toEqual({ table: values });
  });
});
