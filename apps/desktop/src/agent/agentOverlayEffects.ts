import type { WorkspaceAnchor } from "@lattice/agent-protocol";

import {
  type ActiveOverlay,
  type AgentFollowMode,
  shouldRevealViewport,
} from "./agentStore";
import { getAnchorAdapterFor } from "./adapters";
import type { AnchorRevealBehavior } from "./adapters/types";

export type OverlayClearFn = () => void;

export function revealBehaviorForFollowMode(
  followMode: AgentFollowMode,
): AnchorRevealBehavior {
  return shouldRevealViewport(followMode) ? "reveal" : "peek";
}

function applyAnchorOverlay(
  anchor: WorkspaceAnchor,
  overlay: ActiveOverlay,
  behavior: AnchorRevealBehavior,
): OverlayClearFn | undefined {
  const adapter = getAnchorAdapterFor(anchor);
  if (!adapter) {
    return undefined;
  }

  void adapter.reveal(anchor, behavior);
  return adapter.highlight(anchor, {
    overlayId: overlay.overlayId,
    purpose: overlay.purpose,
  });
}

export function applyActiveOverlays(
  activeOverlays: Record<string, ActiveOverlay>,
  followMode: AgentFollowMode,
): OverlayClearFn[] {
  const behavior = revealBehaviorForFollowMode(followMode);
  const clearFns: OverlayClearFn[] = [];

  for (const overlay of Object.values(activeOverlays)) {
    for (const anchor of overlay.anchors) {
      const clear = applyAnchorOverlay(anchor, overlay, behavior);
      if (clear) {
        clearFns.push(clear);
      }
    }
  }

  return clearFns;
}

export function clearOverlayHighlights(clears: readonly OverlayClearFn[]): void {
  for (const clear of clears) {
    clear();
  }
}
