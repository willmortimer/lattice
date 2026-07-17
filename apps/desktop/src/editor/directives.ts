import { parse as parseYaml, stringify as stringifyYaml } from "yaml";

/** Known `:::lattice-embed` fields per docs/07. */
export const LATTICE_EMBED_KNOWN_KEYS = [
  "resource",
  "view",
  "height",
  "lines",
  "fallback",
] as const;

export type LatticeEmbedKnownKey = (typeof LATTICE_EMBED_KNOWN_KEYS)[number];

export interface LatticeEmbedAttrs {
  resource: string;
  view: string | null;
  height: string | null;
  lines: string | null;
  fallback: string | null;
  extraFields: Record<string, string>;
  /** Unknown keys in source order for stable serialization. */
  extraFieldKeys: string[];
}

/** Parse the YAML-like body between directive fences into string fields. */
export function parseDirectiveBody(body: string): Record<string, string> {
  const trimmed = body.trim();
  if (!trimmed) return {};
  const parsed = parseYaml(trimmed);
  if (parsed == null) return {};
  if (typeof parsed !== "object" || Array.isArray(parsed)) {
    throw new Error("Directive body must be a YAML mapping");
  }
  const fields: Record<string, string> = {};
  for (const [key, value] of Object.entries(parsed as Record<string, unknown>)) {
    fields[key] = fieldValueToString(value);
  }
  return fields;
}

function fieldValueToString(value: unknown): string {
  if (value == null) return "";
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  return stringifyYaml(value).trimEnd();
}

/** Split parsed fields into typed lattice-embed attrs, preserving unknown keys. */
export function latticeEmbedAttrsFromFields(fields: Record<string, string>): LatticeEmbedAttrs {
  const known = new Set<string>(LATTICE_EMBED_KNOWN_KEYS);
  const extraFieldKeys: string[] = [];
  const extraFields: Record<string, string> = {};

  for (const key of Object.keys(fields)) {
    if (!known.has(key)) {
      extraFieldKeys.push(key);
      extraFields[key] = fields[key] ?? "";
    }
  }

  return {
    resource: fields.resource ?? "",
    view: fields.view ?? null,
    height: fields.height ?? null,
    lines: fields.lines ?? null,
    fallback: fields.fallback ?? null,
    extraFields,
    extraFieldKeys,
  };
}

function formatDirectiveValue(value: string): string {
  // Markdown-link fallbacks must stay quoted: bare `[label](url)` is invalid YAML
  // (flow sequence + trailing scalar) and breaks parse↔serialize round-trips.
  if (value === "" || /^[\w./_-]+$/.test(value)) {
    return value;
  }
  return JSON.stringify(value);
}

/** Serialize a `:::lattice-embed` block from node attrs. */
export function serializeLatticeEmbed(attrs: LatticeEmbedAttrs): string {
  const lines: string[] = [":::lattice-embed"];

  for (const key of LATTICE_EMBED_KNOWN_KEYS) {
    const value = attrs[key];
    if (value != null && value !== "") {
      lines.push(`${key}: ${formatDirectiveValue(value)}`);
    }
  }

  for (const key of attrs.extraFieldKeys) {
    const value = attrs.extraFields[key];
    if (value != null && value !== "") {
      lines.push(`${key}: ${formatDirectiveValue(value)}`);
    }
  }

  lines.push(":::");
  return `${lines.join("\n")}\n`;
}
