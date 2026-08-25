import type { Doc } from "yjs";

import {
  canvasRelativePath,
  type CanvasAdapter,
  type CanvasSnapshot,
} from "../adapter";
import { buildMaterializedCanvasRaw } from "./canvasMaterialize";
import {
  canvasDataFromYDoc,
  yDocAddEdge,
  yDocAddTextNode,
  yDocMoveNodes,
  yDocPlaceFileNode,
  yDocRemoveEdges,
  yDocRemoveNodes,
  yDocResizeNodes,
  yDocUpdateTextNode,
} from "./canvasYDoc";

export interface CollabCanvasAdapterOptions {
  ydoc: Doc;
  canvasPath: string;
  getRevision: () => string;
  onLocalChange: () => void;
}

/**
 * CanvasAdapter backed by Y.Doc maps. Mutations journal through the collab
 * session; portable `.canvas` JSON is written only by the materializer.
 */
export function createCollabCanvasAdapter(options: CollabCanvasAdapterOptions): CanvasAdapter {
  const commit = (): string => {
    options.onLocalChange();
    return options.getRevision();
  };

  return {
    async read(): Promise<CanvasSnapshot> {
      return {
        content: buildMaterializedCanvasRaw(canvasDataFromYDoc(options.ydoc)),
        revision: options.getRevision(),
      };
    },
    async placeResource(placement) {
      yDocPlaceFileNode(options.ydoc, {
        id: placement.nodeId,
        file: canvasRelativePath(options.canvasPath, placement.resourcePath),
        x: placement.x,
        y: placement.y,
        width: placement.width,
        height: placement.height,
      });
      return commit();
    },
    async moveNodes(nodes) {
      yDocMoveNodes(options.ydoc, nodes);
      return commit();
    },
    async resizeNodes(nodes) {
      yDocResizeNodes(options.ydoc, nodes);
      return commit();
    },
    async removeNodes(nodeIds) {
      yDocRemoveNodes(options.ydoc, nodeIds);
      return commit();
    },
    async removeEdges(edgeIds) {
      yDocRemoveEdges(options.ydoc, edgeIds);
      return commit();
    },
    async addEdge(edge) {
      yDocAddEdge(options.ydoc, {
        id: edge.edgeId,
        fromNode: edge.fromNode,
        toNode: edge.toNode,
        fromSide: edge.fromSide,
        toSide: edge.toSide,
      });
      return commit();
    },
    async addTextNode(placement) {
      yDocAddTextNode(options.ydoc, {
        id: placement.nodeId,
        text: placement.text,
        x: placement.x,
        y: placement.y,
        width: placement.width,
        height: placement.height,
      });
      return commit();
    },
    async updateTextNode(nodeId, text) {
      yDocUpdateTextNode(options.ydoc, nodeId, text);
      return commit();
    },
  };
}
