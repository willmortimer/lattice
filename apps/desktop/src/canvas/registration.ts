import { createNativeCanvasAdapter, type CanvasAdapter } from "./adapter";

/** Per-surface native authority. Browser/demo canvases intentionally receive
 * no filesystem adapter, so the viewer cannot imply native write access. */
export interface CanvasRegistration {
  adapter?: CanvasAdapter;
}

export function registerCanvasSurface(
  workspaceRoot: string | null,
  canvasPath: string,
): CanvasRegistration {
  return workspaceRoot
    ? { adapter: createNativeCanvasAdapter(workspaceRoot, canvasPath) }
    : {};
}
