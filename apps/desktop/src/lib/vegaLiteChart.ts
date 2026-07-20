import type { TopLevelSpec } from "vega-lite";

import type { ArrowFieldMeta } from "./arrowIpc";
import type { VegaRow } from "./arrowToVegaData";

const NUMERIC_TYPES = new Set(["int8", "int16", "int32", "int64", "uint8", "uint16", "uint32", "uint64", "float16", "float32", "float64", "decimal"]);
const NOMINAL_TYPES = new Set(["utf8", "large_utf8", "bool", "boolean"]);

export interface AutoChartColumns {
  xField: string;
  yField: string;
}

/** Pick a simple bar-chart pairing from Arrow schema metadata. */
export function inferAutoChartColumns(schema: ArrowFieldMeta[], values: VegaRow[]): AutoChartColumns | null {
  if (schema.length === 0 || values.length === 0) return null;

  const xField =
    schema.find((field) => NOMINAL_TYPES.has(field.dataType.toLowerCase()))?.name ??
    schema.find((field) => values.some((row) => typeof row[field.name] === "string"))?.name ??
    schema[0]?.name;
  const yField =
    schema.find((field) => field.name !== xField && NUMERIC_TYPES.has(field.dataType.toLowerCase()))?.name ??
    schema.find((field) => field.name !== xField && values.some((row) => typeof row[field.name] === "number"))?.name ??
    null;

  if (!xField || !yField) return null;
  return { xField, yField };
}

export function buildAutoBarChartSpec(schema: ArrowFieldMeta[], values: VegaRow[]): TopLevelSpec | null {
  const columns = inferAutoChartColumns(schema, values);
  if (!columns) return null;

  return {
    $schema: "https://vega.github.io/schema/vega-lite/v6.json",
    description: "Auto-generated bar chart from dataset query results",
    width: "container",
    height: 280,
    data: { values },
    mark: { type: "bar", tooltip: true },
    encoding: {
      x: { field: columns.xField, type: "nominal", title: columns.xField },
      y: { field: columns.yField, type: "quantitative", title: columns.yField },
      color: { field: columns.xField, type: "nominal", legend: null },
    },
  };
}

export function bindValuesToChartSpec(spec: TopLevelSpec, values: VegaRow[]): TopLevelSpec {
  if (values.length === 0) return spec;
  if (isRecord(spec.data) && spec.data.name) {
    return {
      ...spec,
      datasets: {
        ...(spec.datasets ?? {}),
        [spec.data.name]: values,
      },
    };
  }
  return {
    ...spec,
    data: { values },
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
