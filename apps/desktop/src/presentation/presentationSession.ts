import type { DeckSessionDto } from "../lib/deckRun";

/** Future Page and Canvas sequencers share this small host-owned boundary. */
export type PresentationKind = "page" | "deck" | "canvas";
export interface PresentationSession {
  kind: PresentationKind;
  id: string;
  title: string;
  orderedIds: readonly string[];
  initialId: string;
}

/** World-space rectangle used as an explicit camera bookmark. */
export interface CanvasViewportBookmark {
  x: number;
  y: number;
  width: number;
  height: number;
  /** Extra world padding around the rect when framing (default scene padding). */
  padding?: number;
}

/**
 * One presentation beat on a canvas. Prefer `nodeIds` (frame those nodes) or an
 * explicit `viewport`. When only `id` is set and it matches a node, that node
 * is framed.
 */
export interface CanvasSceneSpec {
  id: string;
  title?: string;
  nodeIds?: readonly string[];
  viewport?: CanvasViewportBookmark;
}

/** Sidecar / embedded manifest next to a JSON Canvas (not a `.show` format). */
export interface CanvasPresentationManifest {
  title?: string;
  start?: string;
  scenes: readonly CanvasSceneSpec[];
}

/** Deck is the only registered presentation source in this delivery. */
export function createDeckPresentationSession(deck: DeckSessionDto, anchor?: string | null): PresentationSession {
  const orderedIds = deck.slides.map((slide) => slide.id);
  const initialId = resolveInitialId(orderedIds, anchor, deck.start);
  return { kind: "deck", id: deck.id, title: deck.title, orderedIds, initialId };
}

export function createCanvasPresentationSession(
  canvasId: string,
  title: string,
  scenes: readonly CanvasSceneSpec[],
  options?: { anchor?: string | null; start?: string | null },
): PresentationSession {
  const orderedIds = scenes.map((scene) => scene.id);
  const initialId = resolveInitialId(orderedIds, options?.anchor, options?.start);
  return { kind: "canvas", id: canvasId, title, orderedIds, initialId };
}

export function nearbySlideIndexes(current: number, count: number): number[] {
  return [current - 1, current, current + 1].filter((index) => index >= 0 && index < count);
}

/** Shared index resolution for deck slides and canvas scenes. */
export function resolvePresentationIndex(ids: readonly string[], anchor?: string | null): number {
  const index = anchor ? ids.indexOf(anchor) : -1;
  return index >= 0 ? index : 0;
}

export function resolveDeckSlideIndex(ids: readonly string[], anchor?: string | null): number {
  return resolvePresentationIndex(ids, anchor);
}

export function resolveCanvasSceneIndex(ids: readonly string[], anchor?: string | null): number {
  return resolvePresentationIndex(ids, anchor);
}

/**
 * Build an ordered, usable scene list: sidecar/embedded manifest first, else
 * one scene per non-group node in document order.
 */
export function resolveCanvasScenes(
  manifest: CanvasPresentationManifest | null | undefined,
  nodes: readonly { id: string; type: string }[],
): CanvasSceneSpec[] {
  const nodeIds = new Set(nodes.map((node) => node.id));
  if (manifest?.scenes?.length) {
    const resolved: CanvasSceneSpec[] = [];
    for (const scene of manifest.scenes) {
      if (!scene.id) continue;
      if (scene.viewport && isFiniteViewport(scene.viewport)) {
        resolved.push({
          id: scene.id,
          title: scene.title,
          viewport: scene.viewport,
          nodeIds: scene.nodeIds,
        });
        continue;
      }
      const targets = (scene.nodeIds?.length ? scene.nodeIds : [scene.id]).filter((id) => nodeIds.has(id));
      if (targets.length === 0) continue;
      resolved.push({ id: scene.id, title: scene.title, nodeIds: targets });
    }
    if (resolved.length > 0) return resolved;
  }
  return nodes
    .filter((node) => node.type !== "group")
    .map((node) => ({ id: node.id, nodeIds: [node.id] }));
}

/** Parse a presentation sidecar or embedded metadata object. Throws on bad shape. */
export function parseCanvasPresentationManifest(raw: unknown): CanvasPresentationManifest {
  if (!isRecord(raw)) throw new Error("presentation: expected a JSON object");
  const scenesRaw = raw["scenes"];
  if (!Array.isArray(scenesRaw) || scenesRaw.length === 0) {
    throw new Error('presentation: "scenes" must be a non-empty array');
  }
  const scenes = scenesRaw.map((entry, index) => parseScene(entry, index));
  const title = optionalString(raw, "title");
  const start = optionalString(raw, "start");
  return { title, start, scenes };
}

/**
 * Non-breaking embed: look for `metadata.presentation` or top-level
 * `presentation` on the canvas JSON object (ignored by JSON Canvas parsers).
 */
export function extractEmbeddedCanvasPresentation(raw: unknown): CanvasPresentationManifest | null {
  if (!isRecord(raw)) return null;
  const candidates = [raw["presentation"], isRecord(raw["metadata"]) ? raw["metadata"]["presentation"] : undefined];
  for (const candidate of candidates) {
    if (candidate === undefined) continue;
    try {
      return parseCanvasPresentationManifest(candidate);
    } catch {
      // Invalid embed — fall through to sidecar / node fallback.
    }
  }
  return null;
}

export function canvasPresentationSidecarPath(canvasPath: string): string {
  return `${canvasPath}.presentation.json`;
}

function resolveInitialId(
  orderedIds: readonly string[],
  anchor: string | null | undefined,
  start: string | null | undefined,
): string {
  if (anchor && orderedIds.includes(anchor)) return anchor;
  if (start && orderedIds.includes(start)) return start;
  return orderedIds[0] ?? "";
}

function parseScene(raw: unknown, index: number): CanvasSceneSpec {
  const ctx = `scenes[${index}]`;
  if (!isRecord(raw)) throw new Error(`${ctx}: expected an object`);
  const id = requireString(raw, "id", ctx);
  const title = optionalString(raw, "title");
  let nodeIds: string[] | undefined;
  if (raw["nodeIds"] !== undefined) {
    if (!Array.isArray(raw["nodeIds"]) || !raw["nodeIds"].every((value) => typeof value === "string")) {
      throw new Error(`${ctx}: "nodeIds" must be an array of strings`);
    }
    nodeIds = raw["nodeIds"] as string[];
  }
  let viewport: CanvasViewportBookmark | undefined;
  if (raw["viewport"] !== undefined) {
    if (!isRecord(raw["viewport"])) throw new Error(`${ctx}: "viewport" must be an object`);
    const vp = raw["viewport"];
    viewport = {
      x: requireNumber(vp, "x", `${ctx}.viewport`),
      y: requireNumber(vp, "y", `${ctx}.viewport`),
      width: requireNumber(vp, "width", `${ctx}.viewport`),
      height: requireNumber(vp, "height", `${ctx}.viewport`),
      padding: optionalNumber(vp, "padding", `${ctx}.viewport`),
    };
    if (!isFiniteViewport(viewport)) {
      throw new Error(`${ctx}: "viewport" must have positive finite width/height`);
    }
  }
  return { id, title, nodeIds, viewport };
}

function isFiniteViewport(viewport: CanvasViewportBookmark): boolean {
  return (
    Number.isFinite(viewport.x) &&
    Number.isFinite(viewport.y) &&
    Number.isFinite(viewport.width) &&
    Number.isFinite(viewport.height) &&
    viewport.width > 0 &&
    viewport.height > 0
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function requireString(obj: Record<string, unknown>, key: string, ctx: string): string {
  const value = obj[key];
  if (typeof value !== "string" || value.length === 0) {
    throw new Error(`${ctx}: "${key}" must be a non-empty string`);
  }
  return value;
}

function optionalString(obj: Record<string, unknown>, key: string): string | undefined {
  const value = obj[key];
  if (value === undefined) return undefined;
  if (typeof value !== "string") throw new Error(`presentation: "${key}" must be a string if present`);
  return value;
}

function requireNumber(obj: Record<string, unknown>, key: string, ctx: string): number {
  const value = obj[key];
  if (typeof value !== "number" || !Number.isFinite(value)) {
    throw new Error(`${ctx}: "${key}" must be a finite number`);
  }
  return value;
}

function optionalNumber(obj: Record<string, unknown>, key: string, ctx: string): number | undefined {
  const value = obj[key];
  if (value === undefined) return undefined;
  if (typeof value !== "number" || !Number.isFinite(value)) {
    throw new Error(`${ctx}: "${key}" must be a finite number if present`);
  }
  return value;
}
