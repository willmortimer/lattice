import { invoke } from "@tauri-apps/api/core";
import type { CanvasData, CanvasEdge, CanvasNode, CanvasNodePosition } from "./types";

export type CanvasNodeMove = CanvasNodePosition;

export interface CanvasPlacement {
  resourcePath: string;
  nodeId: string;
  x: number;
  y: number;
  width: number;
  height: number;
  baseRevision: string;
}

export interface CanvasAdapter {
  read(): Promise<CanvasSnapshot>;
  placeResource(placement: CanvasPlacement): Promise<string>;
  moveNodes(nodes: readonly CanvasNodePosition[], baseRevision: string): Promise<string>;
  removeNodes(nodeIds: readonly string[], baseRevision: string): Promise<string>;
  addEdge(edge: CanvasEdgePlacement): Promise<string>;
}

export interface CanvasEdgePlacement {
  edgeId: string;
  fromNode: string;
  toNode: string;
  baseRevision: string;
}

export interface CanvasSnapshot {
  content: string;
  revision: string;
}

export class CanvasStaleRevisionError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "CanvasStaleRevisionError";
  }
}

const STALE_REVISION_PREFIX = "STALE_REVISION:";

/** Convert two workspace-relative paths into the JSON Canvas file reference
 * relative to the canvas file's parent directory. */
export function canvasRelativePath(canvasPath: string, resourcePath: string): string {
  const canvas = splitRelative(canvasPath);
  const resource = splitRelative(resourcePath);
  const parent = canvas.slice(0, -1);
  let common = 0;
  while (common < parent.length && common < resource.length && parent[common] === resource[common]) common += 1;
  return [
    ...Array.from({ length: parent.length - common }, () => ".."),
    ...resource.slice(common),
  ].join("/") || ".";
}

function splitRelative(path: string): string[] {
  if (!path || path.startsWith("/") || path.split("/").some((part) => part === "..")) {
    throw new Error(`path must be workspace-relative: ${path}`);
  }
  return path.split("/").filter((part) => part.length > 0 && part !== ".");
}

function rethrowCanvasError(error: unknown): never {
  const message = String(error);
  if (message.startsWith(STALE_REVISION_PREFIX)) {
    throw new CanvasStaleRevisionError(message.slice(STALE_REVISION_PREFIX.length));
  }
  throw error instanceof Error ? error : new Error(message);
}

export function createNativeCanvasAdapter(root: string, canvasPath: string): CanvasAdapter {
  return {
    async read() {
      return readNativeCanvas(root, canvasPath);
    },
    async placeResource(placement) {
      try {
        const result = await invoke<{ revision: string }>("canvas_place_resource", {
          request: {
            root,
            canvasPath,
            resourcePath: placement.resourcePath,
            nodeId: placement.nodeId,
            x: placement.x,
            y: placement.y,
            width: placement.width,
            height: placement.height,
            baseRevision: placement.baseRevision,
          },
        });
        return result.revision;
      } catch (error) {
        return rethrowCanvasError(error);
      }
    },
    async moveNodes(nodes, baseRevision) {
      try {
        const result = await invoke<{ revision: string }>("canvas_move_nodes", {
          request: { root, canvasPath, nodes, baseRevision },
        });
        return result.revision;
      } catch (error) {
        return rethrowCanvasError(error);
      }
    },
    async removeNodes(nodeIds, baseRevision) {
      try {
        const result = await invoke<{ revision: string }>("canvas_remove_nodes", {
          request: { root, canvasPath, nodeIds, baseRevision },
        });
        return result.revision;
      } catch (error) {
        return rethrowCanvasError(error);
      }
    },
    async addEdge(edge) {
      try {
        const result = await invoke<{ revision: string }>("canvas_add_edge", {
          request: {
            root,
            canvasPath,
            edgeId: edge.edgeId,
            fromNode: edge.fromNode,
            toNode: edge.toNode,
            baseRevision: edge.baseRevision,
          },
        });
        return result.revision;
      } catch (error) {
        return rethrowCanvasError(error);
      }
    },
  };
}

/** Stable short name for renderer registrations and other desktop surfaces. */
export const createCanvasAdapter = createNativeCanvasAdapter;

export async function readNativeCanvas(root: string, canvasPath: string): Promise<CanvasSnapshot> {
  return invoke<CanvasSnapshot>("read_canvas", { root, canvasPath });
}

export function previewMoveNodes(data: CanvasData, moves: readonly CanvasNodeMove[]): CanvasData {
  const byId = new Map(moves.map((move) => [move.id, move]));
  return { ...data, nodes: data.nodes.map((node) => {
    const move = byId.get(node.id);
    return move ? { ...node, x: move.x, y: move.y } : node;
  }) };
}

export function previewRemoveNodes(data: CanvasData, nodeIds: readonly string[]): CanvasData {
  const removed = new Set(nodeIds);
  return {
    ...data,
    nodes: data.nodes.filter((node) => !removed.has(node.id)),
    edges: data.edges.filter((edge) => !removed.has(edge.fromNode) && !removed.has(edge.toNode)),
  };
}

export function previewPlaceResource(
  data: CanvasData,
  resourcePath: string,
  node: Pick<CanvasNode, "id" | "x" | "y" | "width" | "height">,
): CanvasData {
  const placed: CanvasNode = {
    id: node.id, type: "file", file: resourcePath,
    x: node.x, y: node.y, width: node.width, height: node.height,
  };
  return { ...data, nodes: [...data.nodes, placed] };
}

export function previewAddEdge(
  data: CanvasData,
  edge: Pick<CanvasEdge, "id" | "fromNode" | "toNode">,
): CanvasData {
  return { ...data, edges: [...data.edges, { id: edge.id, fromNode: edge.fromNode, toNode: edge.toNode }] };
}

export function keyboardMoveDelta(key: string, shiftKey = false): { x: number; y: number } | null {
  const step = shiftKey ? 10 : 1;
  switch (key) {
    case "ArrowLeft": return { x: -step, y: 0 };
    case "ArrowRight": return { x: step, y: 0 };
    case "ArrowUp": return { x: 0, y: -step };
    case "ArrowDown": return { x: 0, y: step };
    default: return null;
  }
}

export function canvasOutline(data: CanvasData): Array<{ id: string; label: string }> {
  return data.nodes.map((node) => ({
    id: node.id,
    label: node.type === "file" ? node.file : node.type === "text" ? node.text.slice(0, 60) : node.id,
  }));
}
