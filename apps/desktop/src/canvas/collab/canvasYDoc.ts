import * as Y from "yjs";

import type { CanvasData, CanvasEdge, CanvasNode, CanvasNodePosition } from "../types";
import type { CanvasNodeSize } from "../adapter";

/** Shared Y.Doc map of JSON Canvas nodes, keyed by node id. */
export const CANVAS_NODES_KEY = "nodes";
/** Shared Y.Doc map of JSON Canvas edges, keyed by edge id. */
export const CANVAS_EDGES_KEY = "edges";
/** Insertion order for portable `.canvas` `nodes` arrays. */
export const CANVAS_NODE_ORDER_KEY = "nodeOrder";
/** Insertion order for portable `.canvas` `edges` arrays. */
export const CANVAS_EDGE_ORDER_KEY = "edgeOrder";

/** Origin for the initial file → Y.Doc seed so observers can ignore it. */
export const CANVAS_SEED_ORIGIN = "lattice-canvas-seed";

const SIDES = new Set(["top", "right", "bottom", "left"]);

export function canvasNodesMap(ydoc: Y.Doc): Y.Map<Y.Map<unknown>> {
  return ydoc.getMap(CANVAS_NODES_KEY);
}

export function canvasEdgesMap(ydoc: Y.Doc): Y.Map<Y.Map<unknown>> {
  return ydoc.getMap(CANVAS_EDGES_KEY);
}

export function canvasNodeOrder(ydoc: Y.Doc): Y.Array<string> {
  return ydoc.getArray(CANVAS_NODE_ORDER_KEY);
}

export function canvasEdgeOrder(ydoc: Y.Doc): Y.Array<string> {
  return ydoc.getArray(CANVAS_EDGE_ORDER_KEY);
}

export function canvasYDocIsEmpty(ydoc: Y.Doc): boolean {
  return canvasNodesMap(ydoc).size === 0 && canvasEdgesMap(ydoc).size === 0;
}

/**
 * Replace live Y.Doc maps with a JSON Canvas snapshot.
 * Used when opening Collaborative mode on a canvas whose journal is empty.
 */
export function applyCanvasDataToYDoc(ydoc: Y.Doc, data: CanvasData, origin: string = CANVAS_SEED_ORIGIN): void {
  ydoc.transact(() => {
    const nodes = canvasNodesMap(ydoc);
    const edges = canvasEdgesMap(ydoc);
    const nodeOrder = canvasNodeOrder(ydoc);
    const edgeOrder = canvasEdgeOrder(ydoc);

    const nextNodeIds = new Set(data.nodes.map((node) => node.id));
    const nextEdgeIds = new Set(data.edges.map((edge) => edge.id));

    for (const id of Array.from(nodes.keys())) {
      if (!nextNodeIds.has(id)) nodes.delete(id);
    }
    for (const id of Array.from(edges.keys())) {
      if (!nextEdgeIds.has(id)) edges.delete(id);
    }

    for (const node of data.nodes) {
      upsertTypedMap(nodes, node.id, nodeRecord(node));
    }
    for (const edge of data.edges) {
      upsertTypedMap(edges, edge.id, edgeRecord(edge));
    }

    replaceOrder(nodeOrder, data.nodes.map((node) => node.id));
    replaceOrder(edgeOrder, data.edges.map((edge) => edge.id));
  }, origin);
}

/** Materialize the portable JSON Canvas model from live Y.Doc maps. */
export function canvasDataFromYDoc(ydoc: Y.Doc): CanvasData {
  const nodesMap = canvasNodesMap(ydoc);
  const edgesMap = canvasEdgesMap(ydoc);
  const nodeIds = orderedIds(canvasNodeOrder(ydoc), nodesMap);
  const edgeIds = orderedIds(canvasEdgeOrder(ydoc), edgesMap);

  const nodes: CanvasNode[] = [];
  for (const id of nodeIds) {
    const map = nodesMap.get(id);
    if (!map) continue;
    const node = nodeFromYMap(id, map);
    if (node) nodes.push(node);
  }

  const nodeIdSet = new Set(nodes.map((node) => node.id));
  const edges: CanvasEdge[] = [];
  for (const id of edgeIds) {
    const map = edgesMap.get(id);
    if (!map) continue;
    const edge = edgeFromYMap(id, map);
    if (!edge) continue;
    if (!nodeIdSet.has(edge.fromNode) || !nodeIdSet.has(edge.toNode)) continue;
    edges.push(edge);
  }

  return { nodes, edges };
}

export function observeCanvasYDoc(ydoc: Y.Doc, onChange: () => void): () => void {
  const nodes = canvasNodesMap(ydoc);
  const edges = canvasEdgesMap(ydoc);
  const nodeOrder = canvasNodeOrder(ydoc);
  const edgeOrder = canvasEdgeOrder(ydoc);
  const listener = () => {
    onChange();
  };
  nodes.observeDeep(listener);
  edges.observeDeep(listener);
  nodeOrder.observe(listener);
  edgeOrder.observe(listener);
  return () => {
    nodes.unobserveDeep(listener);
    edges.unobserveDeep(listener);
    nodeOrder.unobserve(listener);
    edgeOrder.unobserve(listener);
  };
}

export function yDocPlaceFileNode(
  ydoc: Y.Doc,
  node: Pick<CanvasNode & { type: "file" }, "id" | "file" | "x" | "y" | "width" | "height" | "subpath" | "color">,
): void {
  ydoc.transact(() => {
    const nodes = canvasNodesMap(ydoc);
    if (nodes.has(node.id)) {
      throw new Error(`node id ${JSON.stringify(node.id)} already exists`);
    }
    upsertTypedMap(nodes, node.id, nodeRecord({
      id: node.id,
      type: "file",
      file: node.file,
      x: node.x,
      y: node.y,
      width: node.width,
      height: node.height,
      subpath: node.subpath,
      color: node.color,
    }));
    appendOrder(canvasNodeOrder(ydoc), node.id);
  });
}

export function yDocAddTextNode(
  ydoc: Y.Doc,
  node: Pick<CanvasNode & { type: "text" }, "id" | "text" | "x" | "y" | "width" | "height" | "color">,
): void {
  ydoc.transact(() => {
    const nodes = canvasNodesMap(ydoc);
    if (nodes.has(node.id)) {
      throw new Error(`node id ${JSON.stringify(node.id)} already exists`);
    }
    upsertTypedMap(nodes, node.id, nodeRecord({
      id: node.id,
      type: "text",
      text: node.text,
      x: node.x,
      y: node.y,
      width: node.width,
      height: node.height,
      color: node.color,
    }));
    appendOrder(canvasNodeOrder(ydoc), node.id);
  });
}

export function yDocUpdateTextNode(ydoc: Y.Doc, nodeId: string, text: string): void {
  ydoc.transact(() => {
    const map = requireNodeMap(ydoc, nodeId);
    if (map.get("type") !== "text") {
      throw new Error(`node ${JSON.stringify(nodeId)} is not a text node`);
    }
    map.set("text", text);
  });
}

export function yDocMoveNodes(ydoc: Y.Doc, moves: readonly CanvasNodePosition[]): void {
  ydoc.transact(() => {
    for (const move of moves) {
      const map = requireNodeMap(ydoc, move.id);
      map.set("x", move.x);
      map.set("y", move.y);
    }
  });
}

export function yDocResizeNodes(ydoc: Y.Doc, sizes: readonly CanvasNodeSize[]): void {
  ydoc.transact(() => {
    for (const size of sizes) {
      const map = requireNodeMap(ydoc, size.id);
      map.set("width", size.width);
      map.set("height", size.height);
    }
  });
}

export function yDocRemoveNodes(ydoc: Y.Doc, nodeIds: readonly string[]): void {
  ydoc.transact(() => {
    const nodes = canvasNodesMap(ydoc);
    const edges = canvasEdgesMap(ydoc);
    const removed = new Set(nodeIds);
    for (const id of nodeIds) {
      nodes.delete(id);
    }
    removeFromOrder(canvasNodeOrder(ydoc), removed);
    for (const [edgeId, edge] of edges.entries()) {
      const from = edge.get("fromNode");
      const to = edge.get("toNode");
      if ((typeof from === "string" && removed.has(from)) || (typeof to === "string" && removed.has(to))) {
        edges.delete(edgeId);
      }
    }
    const remainingEdges = new Set(edges.keys());
    removeFromOrder(canvasEdgeOrder(ydoc), new Set(
      Array.from(canvasEdgeOrder(ydoc)).filter((id) => !remainingEdges.has(id)),
    ));
  });
}

export function yDocAddEdge(
  ydoc: Y.Doc,
  edge: Pick<CanvasEdge, "id" | "fromNode" | "toNode" | "fromSide" | "toSide" | "label" | "color">,
): void {
  ydoc.transact(() => {
    const nodes = canvasNodesMap(ydoc);
    if (!nodes.has(edge.fromNode)) {
      throw new Error(`fromNode ${JSON.stringify(edge.fromNode)} does not exist`);
    }
    if (!nodes.has(edge.toNode)) {
      throw new Error(`toNode ${JSON.stringify(edge.toNode)} does not exist`);
    }
    const edges = canvasEdgesMap(ydoc);
    if (edges.has(edge.id)) {
      throw new Error(`edge id ${JSON.stringify(edge.id)} already exists`);
    }
    upsertTypedMap(edges, edge.id, edgeRecord({
      id: edge.id,
      fromNode: edge.fromNode,
      toNode: edge.toNode,
      fromSide: edge.fromSide,
      toSide: edge.toSide,
      label: edge.label,
      color: edge.color,
    }));
    appendOrder(canvasEdgeOrder(ydoc), edge.id);
  });
}

export function yDocRemoveEdges(ydoc: Y.Doc, edgeIds: readonly string[]): void {
  ydoc.transact(() => {
    const edges = canvasEdgesMap(ydoc);
    for (const id of edgeIds) {
      edges.delete(id);
    }
    removeFromOrder(canvasEdgeOrder(ydoc), new Set(edgeIds));
  });
}

function requireNodeMap(ydoc: Y.Doc, nodeId: string): Y.Map<unknown> {
  const map = canvasNodesMap(ydoc).get(nodeId);
  if (!map) {
    throw new Error(`node id ${JSON.stringify(nodeId)} does not exist`);
  }
  return map;
}

function upsertTypedMap(
  parent: Y.Map<Y.Map<unknown>>,
  id: string,
  record: Record<string, string | number>,
): void {
  let map = parent.get(id);
  if (!map) {
    map = new Y.Map<unknown>();
    parent.set(id, map);
  }
  const keep = new Set(Object.keys(record));
  for (const key of Array.from(map.keys())) {
    if (!keep.has(key)) map.delete(key);
  }
  for (const [key, value] of Object.entries(record)) {
    if (map.get(key) !== value) {
      map.set(key, value);
    }
  }
}

function replaceOrder(order: Y.Array<string>, ids: readonly string[]): void {
  if (order.length > 0) {
    order.delete(0, order.length);
  }
  if (ids.length > 0) {
    order.push(ids.slice());
  }
}

function appendOrder(order: Y.Array<string>, id: string): void {
  if (!Array.from(order).includes(id)) {
    order.push([id]);
  }
}

function removeFromOrder(order: Y.Array<string>, ids: ReadonlySet<string>): void {
  for (let index = order.length - 1; index >= 0; index -= 1) {
    const current = order.get(index);
    if (current !== undefined && ids.has(current)) {
      order.delete(index, 1);
    }
  }
}

function orderedIds(
  order: Y.Array<string>,
  map: { has(id: string): boolean; keys(): IterableIterator<string> },
): string[] {
  const seen = new Set<string>();
  const ids: string[] = [];
  for (const id of order) {
    if (typeof id !== "string" || seen.has(id) || !map.has(id)) continue;
    seen.add(id);
    ids.push(id);
  }
  for (const id of map.keys()) {
    if (seen.has(id)) continue;
    ids.push(id);
  }
  return ids;
}

function nodeRecord(node: CanvasNode): Record<string, string | number> {
  const record: Record<string, string | number> = {
    id: node.id,
    type: node.type,
    x: node.x,
    y: node.y,
    width: node.width,
    height: node.height,
  };
  if (node.color !== undefined) record.color = node.color;
  switch (node.type) {
    case "text":
      record.text = node.text;
      break;
    case "file":
      record.file = node.file;
      if (node.subpath !== undefined) record.subpath = node.subpath;
      break;
    case "link":
      record.url = node.url;
      break;
    case "group":
      if (node.label !== undefined) record.label = node.label;
      break;
    default: {
      const neverNode: never = node;
      return neverNode;
    }
  }
  return record;
}

function edgeRecord(edge: CanvasEdge): Record<string, string | number> {
  const record: Record<string, string | number> = {
    id: edge.id,
    fromNode: edge.fromNode,
    toNode: edge.toNode,
  };
  if (edge.fromSide !== undefined) record.fromSide = edge.fromSide;
  if (edge.toSide !== undefined) record.toSide = edge.toSide;
  if (edge.label !== undefined) record.label = edge.label;
  if (edge.color !== undefined) record.color = edge.color;
  return record;
}

function nodeFromYMap(id: string, map: Y.Map<unknown>): CanvasNode | null {
  const type = map.get("type");
  const x = asFiniteNumber(map.get("x"));
  const y = asFiniteNumber(map.get("y"));
  const width = asFiniteNumber(map.get("width"));
  const height = asFiniteNumber(map.get("height"));
  if (x === null || y === null || width === null || height === null) return null;
  const color = asOptionalString(map.get("color"));
  const resolvedId = asOptionalString(map.get("id")) ?? id;

  switch (type) {
    case "text": {
      const text = asOptionalString(map.get("text"));
      if (text === undefined) return null;
      return { id: resolvedId, type, x, y, width, height, color, text };
    }
    case "file": {
      const file = asOptionalString(map.get("file"));
      if (file === undefined) return null;
      return {
        id: resolvedId,
        type,
        x,
        y,
        width,
        height,
        color,
        file,
        subpath: asOptionalString(map.get("subpath")),
      };
    }
    case "link": {
      const url = asOptionalString(map.get("url"));
      if (url === undefined) return null;
      return { id: resolvedId, type, x, y, width, height, color, url };
    }
    case "group":
      return {
        id: resolvedId,
        type,
        x,
        y,
        width,
        height,
        color,
        label: asOptionalString(map.get("label")),
      };
    default:
      return null;
  }
}

function edgeFromYMap(id: string, map: Y.Map<unknown>): CanvasEdge | null {
  const fromNode = asOptionalString(map.get("fromNode"));
  const toNode = asOptionalString(map.get("toNode"));
  if (fromNode === undefined || toNode === undefined) return null;
  return {
    id: asOptionalString(map.get("id")) ?? id,
    fromNode,
    toNode,
    fromSide: asOptionalSide(map.get("fromSide")),
    toSide: asOptionalSide(map.get("toSide")),
    label: asOptionalString(map.get("label")),
    color: asOptionalString(map.get("color")),
  };
}

function asFiniteNumber(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function asOptionalString(value: unknown): string | undefined {
  return typeof value === "string" ? value : undefined;
}

function asOptionalSide(value: unknown): CanvasEdge["fromSide"] {
  if (typeof value !== "string" || !SIDES.has(value)) return undefined;
  return value as CanvasEdge["fromSide"];
}
