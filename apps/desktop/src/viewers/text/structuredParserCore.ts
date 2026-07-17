import { isAlias, isMap, isScalar, isSeq, parseDocument } from "yaml";

export type StructuredSyntax = "json" | "yaml";

export interface StructuredParseLimits {
  maxDepth: number;
  maxNodes: number;
  maxAliases: number;
}

export const DEFAULT_STRUCTURED_PARSE_LIMITS: StructuredParseLimits = {
  maxDepth: 128,
  maxNodes: 50_000,
  maxAliases: 100,
};

export type StructuredNode =
  | { kind: "object"; entries: Array<{ key: string; value: StructuredNode }> }
  | { kind: "array"; items: StructuredNode[] }
  | { kind: "value"; value: string | number | boolean | null }
  | { kind: "alias"; name: string };

export interface StructuredDiagnostic {
  message: string;
  line?: number;
  column?: number;
}

export type StructuredParseResult =
  | { ok: true; root: StructuredNode; diagnostics: [] }
  | { ok: false; root: null; diagnostics: StructuredDiagnostic[] };

class ParseLimitError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "ParseLimitError";
  }
}

function limitFor(limits: Partial<StructuredParseLimits> | undefined): StructuredParseLimits {
  return { ...DEFAULT_STRUCTURED_PARSE_LIMITS, ...limits };
}

function objectKey(value: unknown): string {
  if (typeof value === "string") return value;
  if (value === null) return "null";
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  return "[complex key]";
}

function normalizeJson(value: unknown, depth: number, state: { nodes: number }, limits: StructuredParseLimits): StructuredNode {
  state.nodes += 1;
  if (state.nodes > limits.maxNodes) throw new ParseLimitError(`Structured value exceeds the ${limits.maxNodes.toLocaleString()} node limit.`);
  if (depth > limits.maxDepth) throw new ParseLimitError(`Structured value exceeds the depth limit of ${limits.maxDepth}.`);
  if (value === null || typeof value === "string" || typeof value === "number" || typeof value === "boolean") {
    return { kind: "value", value };
  }
  if (Array.isArray(value)) {
    return { kind: "array", items: value.map((item) => normalizeJson(item, depth + 1, state, limits)) };
  }
  return {
    kind: "object",
    entries: Object.entries(value as Record<string, unknown>).map(([key, item]) => ({
      key,
      value: normalizeJson(item, depth + 1, state, limits),
    })),
  };
}

function scalarKey(value: unknown): string {
  if (isScalar(value)) {
    const scalar = value.value;
    if (typeof scalar === "string") return scalar;
    if (scalar === null) return "null";
    if (typeof scalar === "number" || typeof scalar === "boolean") return String(scalar);
  }
  return objectKey(value);
}

/** Converts YAML CST nodes without resolving aliases. This keeps the tree bounded
 * and prevents a recursive alias graph from becoming an expanded JS object. */
function normalizeYaml(node: unknown, depth: number, state: { nodes: number; aliases: number }, limits: StructuredParseLimits): StructuredNode {
  state.nodes += 1;
  if (state.nodes > limits.maxNodes) throw new ParseLimitError(`Structured value exceeds the ${limits.maxNodes.toLocaleString()} node limit.`);
  if (depth > limits.maxDepth) throw new ParseLimitError(`Structured value exceeds the depth limit of ${limits.maxDepth}.`);
  if (isAlias(node)) {
    state.aliases += 1;
    if (state.aliases > limits.maxAliases) throw new ParseLimitError(`YAML exceeds the ${limits.maxAliases} alias limit.`);
    return { kind: "alias", name: String(node.source ?? "alias") };
  }
  if (isMap(node)) {
    return {
      kind: "object",
      entries: node.items.map((item) => ({
        key: scalarKey(item.key),
        value: normalizeYaml(item.value, depth + 1, state, limits),
      })),
    };
  }
  if (isSeq(node)) {
    return { kind: "array", items: node.items.map((item) => normalizeYaml(item, depth + 1, state, limits)) };
  }
  if (isScalar(node)) {
    const value = node.value;
    if (value === null || typeof value === "string" || typeof value === "number" || typeof value === "boolean") {
      return { kind: "value", value };
    }
    return { kind: "value", value: String(value) };
  }
  return { kind: "value", value: null };
}

function diagnosticFromError(error: unknown): StructuredDiagnostic {
  const message = error instanceof Error ? error.message : String(error);
  const match = message.match(/line (\d+), column (\d+)/i);
  return match
    ? { message, line: Number(match[1]), column: Number(match[2]) }
    : { message };
}

export function parseStructuredSource(
  source: string,
  syntax: StructuredSyntax,
  requestedLimits?: Partial<StructuredParseLimits>,
): StructuredParseResult {
  const limits = limitFor(requestedLimits);
  try {
    if (syntax === "json") return { ok: true, root: normalizeJson(JSON.parse(source), 0, { nodes: 0 }, limits), diagnostics: [] };

    const document = parseDocument(source, {
      prettyErrors: false,
      customTags: [],
    });
    if (document.errors.length > 0) {
      return { ok: false, root: null, diagnostics: document.errors.map(diagnosticFromError) };
    }
    return {
      ok: true,
      root: normalizeYaml(document.contents, 0, { nodes: 0, aliases: 0 }, limits),
      diagnostics: [],
    };
  } catch (error) {
    return { ok: false, root: null, diagnostics: [diagnosticFromError(error)] };
  }
}
