import { describe, expect, it } from "vitest";

import { looksLikeLatticeResourceId, type CatalogEntry } from "../../lib/resourceCatalog";
import { shouldPatchPlainCanvas } from "./canvasMaterialize";
import {
  canvasCollaborativeAvailable,
  canvasEditAdapterKind,
  resolveCanvasRegistryResourceId,
  shouldOpenCanvasCollabSession,
  shouldRefuseCanvasCollaborative,
} from "./canvasCollabContract";

const REGISTRY_ID = "550e8400-e29b-41d4-a716-446655440000";
const CANVAS_PATH = "Boards/Map.canvas";

function catalogWith(resourceId: string, path = CANVAS_PATH): Map<string, CatalogEntry> {
  return new Map([
    [
      resourceId,
      {
        resourceId,
        path,
        kind: "canvas",
        childCount: 0,
      },
    ],
  ]);
}

describe("canvas collab renderer contract", () => {
  it("refuses Collaborative without a registry ResourceId", () => {
    expect(looksLikeLatticeResourceId("path:Boards/Map.canvas")).toBe(false);
    expect(looksLikeLatticeResourceId(REGISTRY_ID)).toBe(true);

    expect(canvasCollaborativeAvailable(undefined)).toBe(false);
    expect(canvasCollaborativeAvailable("path:Boards/Map.canvas")).toBe(false);
    expect(canvasCollaborativeAvailable("not-a-uuid")).toBe(false);
    expect(canvasCollaborativeAvailable(REGISTRY_ID)).toBe(true);

    expect(shouldRefuseCanvasCollaborative("collaborative", undefined)).toBe(true);
    expect(shouldRefuseCanvasCollaborative("collaborative", "path:Boards/Map.canvas")).toBe(true);
    expect(shouldRefuseCanvasCollaborative("collaborative", REGISTRY_ID)).toBe(false);
    expect(shouldRefuseCanvasCollaborative("plain", undefined)).toBe(false);
  });

  it("does not open a collab session without a registry ResourceId", () => {
    expect(shouldOpenCanvasCollabSession("collaborative", undefined)).toBe(false);
    expect(shouldOpenCanvasCollabSession("collaborative", "path:Boards/Map.canvas")).toBe(false);
    expect(shouldOpenCanvasCollabSession("plain", REGISTRY_ID)).toBe(false);
    expect(shouldOpenCanvasCollabSession("collaborative", REGISTRY_ID)).toBe(true);
  });

  it("ignores synthetic catalog ids when resolving the registry ResourceId", () => {
    expect(
      resolveCanvasRegistryResourceId(catalogWith("path:Boards/Map.canvas"), CANVAS_PATH),
    ).toBeUndefined();
    expect(resolveCanvasRegistryResourceId(catalogWith(REGISTRY_ID), CANVAS_PATH)).toBe(REGISTRY_ID);
    expect(resolveCanvasRegistryResourceId(catalogWith(REGISTRY_ID), "Other.canvas")).toBeUndefined();
    expect(resolveCanvasRegistryResourceId(new Map(), CANVAS_PATH)).toBeUndefined();
  });

  it("does not patch the .canvas file per gesture in collaborative mode", () => {
    expect(shouldPatchPlainCanvas("collaborative")).toBe(false);
    expect(shouldPatchPlainCanvas("plain")).toBe(true);
    expect(canvasEditAdapterKind("collaborative")).toBe("collab");
    expect(canvasEditAdapterKind("plain")).toBe("native");
  });
});
