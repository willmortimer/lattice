import type { TopLevelSpec } from "vega-lite";

export interface LatticeChartDataBinding {
  /** Workspace-relative `.dataset` package path. */
  dataset: string;
  sql?: string;
  maxRows?: number;
}

export interface LatticeChartDocument {
  lattice?: {
    data?: LatticeChartDataBinding;
  };
}

export interface ParsedChartSpec {
  spec: TopLevelSpec;
  binding: LatticeChartDataBinding | null;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** Parse a `.vl.json` document, extracting the Lattice data binding when present. */
export function parseChartSpecDocument(raw: unknown): ParsedChartSpec {
  if (!isRecord(raw)) {
    throw new Error("Chart spec must be a JSON object");
  }

  const lattice = isRecord(raw.lattice) ? raw.lattice : null;
  const dataBinding = parseDataBinding(lattice?.data);
  const { lattice: _lattice, ...spec } = raw;
  return {
    spec: spec as unknown as TopLevelSpec,
    binding: dataBinding,
  };
}

function parseDataBinding(value: unknown): LatticeChartDataBinding | null {
  if (!isRecord(value) || typeof value.dataset !== "string" || value.dataset.trim().length === 0) {
    return null;
  }
  const binding: LatticeChartDataBinding = { dataset: value.dataset.trim() };
  if (typeof value.sql === "string" && value.sql.trim().length > 0) {
    binding.sql = value.sql.trim();
  }
  if (typeof value.maxRows === "number" && Number.isFinite(value.maxRows) && value.maxRows > 0) {
    binding.maxRows = Math.floor(value.maxRows);
  }
  return binding;
}

export function parseChartSpecText(text: string): ParsedChartSpec {
  let parsed: unknown;
  try {
    parsed = JSON.parse(text);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(`Invalid chart JSON: ${message}`);
  }
  return parseChartSpecDocument(parsed);
}
