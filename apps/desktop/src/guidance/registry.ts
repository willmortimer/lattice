import type { GuidanceAnchor } from "./types";

const anchors = new Map<string, GuidanceAnchor>();

export function registerGuidanceAnchor(anchor: GuidanceAnchor): () => void {
  anchors.set(anchor.id, anchor);
  return () => {
    const current = anchors.get(anchor.id);
    if (current === anchor) {
      anchors.delete(anchor.id);
    }
  };
}

export function getGuidanceAnchor(id: string): GuidanceAnchor | undefined {
  return anchors.get(id);
}

export function listGuidanceAnchors(): GuidanceAnchor[] {
  return [...anchors.values()];
}

export function clearGuidanceAnchors(): void {
  anchors.clear();
}
