// JSON Canvas data model (https://jsoncanvas.org). Read-only subset: enough
// to lay out and render nodes/edges, nothing about the editing extensions.

export type CanvasNodeType = "text" | "file" | "link" | "group";

interface BaseNode {
  id: string;
  x: number;
  y: number;
  width: number;
  height: number;
  /** Either a hex color ("#rrggbb") or a JSON Canvas preset 1-6. Optional. */
  color?: string;
}

export interface TextNode extends BaseNode {
  type: "text";
  text: string;
}

export interface FileNode extends BaseNode {
  type: "file";
  file: string;
  subpath?: string;
}

export interface LinkNode extends BaseNode {
  type: "link";
  url: string;
}

export interface GroupNode extends BaseNode {
  type: "group";
  label?: string;
}

export type CanvasNode = TextNode | FileNode | LinkNode | GroupNode;

export interface CanvasEdge {
  id: string;
  fromNode: string;
  toNode: string;
  fromSide?: "top" | "right" | "bottom" | "left";
  toSide?: "top" | "right" | "bottom" | "left";
  label?: string;
  color?: string;
}

export interface CanvasData {
  nodes: CanvasNode[];
  edges: CanvasEdge[];
}

/** Thrown by parseCanvas with a human-readable, specific message. */
export class CanvasParseError extends Error {}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function requireNumber(obj: Record<string, unknown>, key: string, ctx: string): number {
  const v = obj[key];
  if (typeof v !== "number" || !Number.isFinite(v)) {
    throw new CanvasParseError(`${ctx}: "${key}" must be a finite number`);
  }
  return v;
}

function requireString(obj: Record<string, unknown>, key: string, ctx: string): string {
  const v = obj[key];
  if (typeof v !== "string") {
    throw new CanvasParseError(`${ctx}: "${key}" must be a string`);
  }
  return v;
}

function optionalString(obj: Record<string, unknown>, key: string, ctx: string): string | undefined {
  const v = obj[key];
  if (v === undefined) return undefined;
  if (typeof v !== "string") {
    throw new CanvasParseError(`${ctx}: "${key}" must be a string if present`);
  }
  return v;
}

const SIDES = new Set(["top", "right", "bottom", "left"]);

function optionalSide(
  obj: Record<string, unknown>,
  key: string,
  ctx: string,
): CanvasEdge["fromSide"] {
  const v = obj[key];
  if (v === undefined) return undefined;
  if (typeof v !== "string" || !SIDES.has(v)) {
    throw new CanvasParseError(`${ctx}: "${key}" must be one of top/right/bottom/left`);
  }
  return v as CanvasEdge["fromSide"];
}

function parseNode(raw: unknown, index: number): CanvasNode {
  const ctx = `nodes[${index}]`;
  if (!isRecord(raw)) throw new CanvasParseError(`${ctx}: expected an object`);

  const id = requireString(raw, "id", ctx);
  const type = raw["type"];
  const x = requireNumber(raw, "x", ctx);
  const y = requireNumber(raw, "y", ctx);
  const width = requireNumber(raw, "width", ctx);
  const height = requireNumber(raw, "height", ctx);
  const color = optionalString(raw, "color", ctx);

  switch (type) {
    case "text":
      return { id, type: "text", x, y, width, height, color, text: requireString(raw, "text", ctx) };
    case "file":
      return {
        id,
        type: "file",
        x,
        y,
        width,
        height,
        color,
        file: requireString(raw, "file", ctx),
        subpath: optionalString(raw, "subpath", ctx),
      };
    case "link":
      return { id, type: "link", x, y, width, height, color, url: requireString(raw, "url", ctx) };
    case "group":
      return { id, type: "group", x, y, width, height, color, label: optionalString(raw, "label", ctx) };
    default:
      throw new CanvasParseError(`${ctx}: unknown node "type" ${JSON.stringify(type)}`);
  }
}

function parseEdge(raw: unknown, index: number): CanvasEdge {
  const ctx = `edges[${index}]`;
  if (!isRecord(raw)) throw new CanvasParseError(`${ctx}: expected an object`);

  return {
    id: requireString(raw, "id", ctx),
    fromNode: requireString(raw, "fromNode", ctx),
    toNode: requireString(raw, "toNode", ctx),
    fromSide: optionalSide(raw, "fromSide", ctx),
    toSide: optionalSide(raw, "toSide", ctx),
    label: optionalString(raw, "label", ctx),
    color: optionalString(raw, "color", ctx),
  };
}

/** Parse and validate a JSON Canvas document. Throws CanvasParseError. */
export function parseCanvas(raw: unknown): CanvasData {
  if (!isRecord(raw)) throw new CanvasParseError("canvas: expected a JSON object");

  const nodesRaw = raw["nodes"];
  const edgesRaw = raw["edges"] ?? [];
  if (!Array.isArray(nodesRaw)) throw new CanvasParseError('canvas: "nodes" must be an array');
  if (!Array.isArray(edgesRaw)) throw new CanvasParseError('canvas: "edges" must be an array');

  const nodes = nodesRaw.map(parseNode);
  const edges = edgesRaw.map(parseEdge);

  const ids = new Set(nodes.map((n) => n.id));
  for (const edge of edges) {
    if (!ids.has(edge.fromNode)) {
      throw new CanvasParseError(`edge "${edge.id}": fromNode "${edge.fromNode}" does not exist`);
    }
    if (!ids.has(edge.toNode)) {
      throw new CanvasParseError(`edge "${edge.id}": toNode "${edge.toNode}" does not exist`);
    }
  }

  return { nodes, edges };
}
