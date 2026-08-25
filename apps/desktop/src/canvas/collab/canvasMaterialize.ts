import type { Doc } from "yjs";

import type { PagePersistMode } from "../../editor/collab/collabSession";
import { shouldScheduleCollabCheckpoint } from "../../editor/collab/collabMaterialize";
import { StaleRevisionError } from "../../editor/pageIO";
import { applyResourceUpdate } from "../../lib/resourceRuntime";
import { CanvasStaleRevisionError } from "../adapter";
import type { CanvasData } from "../types";
import { canvasDataFromYDoc } from "./canvasYDoc";

export { shouldScheduleCollabCheckpoint };

const STALE_REVISION_PREFIX = "STALE_REVISION:";

/** Portable JSON Canvas bytes for a collaborative checkpoint. */
export function buildMaterializedCanvasRaw(data: CanvasData): string {
  return `${JSON.stringify(
    {
      nodes: data.nodes,
      edges: data.edges,
    },
    null,
    2,
  )}\n`;
}

export interface CanvasFileIO {
  save(raw: string, baseRevision: string | null): Promise<string | null>;
}

export function createNativeCanvasFileIO(root: string, canvasPath: string): CanvasFileIO {
  return {
    async save(raw, baseRevision) {
      try {
        return await applyResourceUpdate({
          root,
          path: canvasPath,
          content: new TextEncoder().encode(raw),
          baseRevision: baseRevision ?? "",
        });
      } catch (error) {
        const message = String(error);
        if (message.startsWith(STALE_REVISION_PREFIX) || message.includes("STALE_REVISION")) {
          const detail = message.startsWith(STALE_REVISION_PREFIX)
            ? message.slice(STALE_REVISION_PREFIX.length)
            : message;
          throw new CanvasStaleRevisionError(detail);
        }
        throw error instanceof Error ? error : new Error(message);
      }
    },
  };
}

export interface CollabMaterializeCanvasDeps {
  ydoc: Doc;
  io: CanvasFileIO;
}

/** Persist the current Y.Doc canvas as portable `.canvas` JSON. */
export async function materializeCollabCanvas(
  deps: CollabMaterializeCanvasDeps,
  baseRevision: string | null,
): Promise<string | null> {
  const data = canvasDataFromYDoc(deps.ydoc);
  const raw = buildMaterializedCanvasRaw(data);
  return deps.io.save(raw, baseRevision);
}

export function isCanvasMaterializeConflict(error: unknown): boolean {
  return error instanceof CanvasStaleRevisionError || error instanceof StaleRevisionError;
}

/** Collaborative canvas edits must not use per-gesture file patches. */
export function shouldPatchPlainCanvas(mode: PagePersistMode): boolean {
  return mode === "plain";
}

export type { PagePersistMode };
