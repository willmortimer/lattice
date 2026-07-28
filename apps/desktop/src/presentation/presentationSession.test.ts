import { describe, expect, it } from "vitest";
import {
  createCanvasPresentationSession,
  extractEmbeddedCanvasPresentation,
  nearbySlideIndexes,
  parseCanvasPresentationManifest,
  resolveCanvasSceneIndex,
  resolveCanvasScenes,
  resolveDeckSlideIndex,
  resolvePresentationIndex,
} from "./presentationSession";

describe("presentation session helpers", () => {
  it("mounts only the current slide and immediate neighbours", () => {
    expect(nearbySlideIndexes(0, 4)).toEqual([0, 1]);
    expect(nearbySlideIndexes(2, 4)).toEqual([1, 2, 3]);
  });
  it("honours a valid deep-link anchor", () => {
    expect(resolveDeckSlideIndex(["title", "summary"], "summary")).toBe(1);
    expect(resolveDeckSlideIndex(["title", "summary"], "missing")).toBe(0);
  });
  it("resolves canvas scene indexes like deck slides", () => {
    expect(resolveCanvasSceneIndex(["overview", "thesis"], "thesis")).toBe(1);
    expect(resolvePresentationIndex(["a", "b"], null)).toBe(0);
  });
});

describe("canvas presentation sequencer", () => {
  const nodes = [
    { id: "thesis", type: "text" },
    { id: "product", type: "file" },
    { id: "cluster", type: "group" },
    { id: "docs", type: "file" },
  ];

  it("falls back to non-group nodes in document order", () => {
    expect(resolveCanvasScenes(null, nodes)).toEqual([
      { id: "thesis", nodeIds: ["thesis"] },
      { id: "product", nodeIds: ["product"] },
      { id: "docs", nodeIds: ["docs"] },
    ]);
  });

  it("keeps manifest scenes that frame known nodes or viewports", () => {
    const scenes = resolveCanvasScenes(
      {
        start: "product",
        scenes: [
          { id: "overview", nodeIds: ["thesis", "product", "missing"] },
          { id: "orphan", nodeIds: ["gone"] },
          { id: "bookmark", viewport: { x: 0, y: 0, width: 400, height: 300 } },
          { id: "product" },
        ],
      },
      nodes,
    );
    expect(scenes.map((scene) => scene.id)).toEqual(["overview", "bookmark", "product"]);
    expect(scenes[0]?.nodeIds).toEqual(["thesis", "product"]);
    expect(scenes[2]?.nodeIds).toEqual(["product"]);
  });

  it("builds a canvas PresentationSession from resolved scenes", () => {
    const scenes = resolveCanvasScenes(
      { start: "docs", scenes: [{ id: "thesis" }, { id: "docs" }] },
      nodes,
    );
    const session = createCanvasPresentationSession("Hackathon/Pitch.canvas", "Pitch", scenes, {
      start: "docs",
    });
    expect(session.kind).toBe("canvas");
    expect(session.orderedIds).toEqual(["thesis", "docs"]);
    expect(session.initialId).toBe("docs");
    expect(resolveCanvasSceneIndex(session.orderedIds, session.initialId)).toBe(1);
  });

  it("honours createCanvasPresentationSession anchor over document order", () => {
    const session = createCanvasPresentationSession(
      "c",
      "Pitch",
      [{ id: "a" }, { id: "b" }],
      { anchor: "b" },
    );
    expect(session.initialId).toBe("b");
    expect(resolveCanvasSceneIndex(session.orderedIds, session.initialId)).toBe(1);
  });

  it("parses a sidecar manifest and embedded metadata", () => {
    const manifest = parseCanvasPresentationManifest({
      title: "Pitch",
      start: "thesis",
      scenes: [
        { id: "overview", title: "All", nodeIds: ["thesis", "product"] },
        { id: "zoom", viewport: { x: 10, y: 20, width: 100, height: 80, padding: 24 } },
      ],
    });
    expect(manifest.title).toBe("Pitch");
    expect(manifest.start).toBe("thesis");
    expect(manifest.scenes).toHaveLength(2);

    expect(
      extractEmbeddedCanvasPresentation({
        nodes: [],
        edges: [],
        metadata: { presentation: { scenes: [{ id: "thesis" }] } },
      })?.scenes[0]?.id,
    ).toBe("thesis");
    expect(
      extractEmbeddedCanvasPresentation({
        presentation: { scenes: [{ id: "docs" }] },
      })?.scenes[0]?.id,
    ).toBe("docs");
    expect(extractEmbeddedCanvasPresentation({ nodes: [] })).toBeNull();
  });

  it("rejects empty or malformed manifests", () => {
    expect(() => parseCanvasPresentationManifest({ scenes: [] })).toThrow(/non-empty/);
    expect(() => parseCanvasPresentationManifest({ scenes: [{ id: 1 }] })).toThrow(/id/);
  });
});
