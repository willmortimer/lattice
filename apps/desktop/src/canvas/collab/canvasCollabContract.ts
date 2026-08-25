import type { PagePersistMode } from "../../editor/collab/collabSession";
import {
  looksLikeLatticeResourceId,
  resourceIdForPath,
  type CatalogEntry,
} from "../../lib/resourceCatalog";
import { shouldPatchPlainCanvas } from "./canvasMaterialize";

export function resolveCanvasRegistryResourceId(
  catalog: ReadonlyMap<string, CatalogEntry>,
  path: string,
): string | undefined {
  const catalogId = resourceIdForPath(catalog, path);
  if (catalogId && looksLikeLatticeResourceId(catalogId)) {
    return catalogId;
  }
  return undefined;
}

/** Collaborative chrome and Yrs session require a registry UUID, not a path: placeholder. */
export function canvasCollaborativeAvailable(
  registryResourceId: string | undefined,
): registryResourceId is string {
  return (
    registryResourceId !== undefined &&
    looksLikeLatticeResourceId(registryResourceId)
  );
}

/** Persist-mode toggle must no-op when Collaborative is requested without a registry UUID. */
export function shouldRefuseCanvasCollaborative(
  mode: PagePersistMode,
  registryResourceId: string | undefined,
): boolean {
  return mode === "collaborative" && !canvasCollaborativeAvailable(registryResourceId);
}

export function shouldOpenCanvasCollabSession(
  persistMode: PagePersistMode,
  registryResourceId: string | undefined,
): boolean {
  return persistMode === "collaborative" && canvasCollaborativeAvailable(registryResourceId);
}

export type CanvasEditAdapterKind = "native" | "collab";

/** Plain-file canvases keep the native patch adapter; Collaborative uses the Y.Doc adapter. */
export function canvasEditAdapterKind(persistMode: PagePersistMode): CanvasEditAdapterKind {
  return shouldPatchPlainCanvas(persistMode) ? "native" : "collab";
}
