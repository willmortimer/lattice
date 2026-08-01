import type { WorkspaceAnchor } from "@lattice/agent-protocol";

import { getAnchorAdapterFor } from "../agent/adapters";
import type { ActiveOverlay } from "../agent/agentStore";
import { elementRect } from "../guidance/domAnchor";

export function resolveAnchorScreenRect(anchor: WorkspaceAnchor): DOMRect | null {
  const adapter = getAnchorAdapterFor(anchor);
  return adapter?.getScreenRect?.(anchor) ?? null;
}

export function resolveOverlayAnchorRect(overlay: ActiveOverlay): DOMRect | null {
  for (const anchor of overlay.anchors) {
    const rect = resolveAnchorScreenRect(anchor);
    if (rect) return rect;
  }

  const element = document.querySelector(
    `[data-agent-overlay-id="${overlay.overlayId}"]`,
  ) as HTMLElement | null;
  return elementRect(element);
}
