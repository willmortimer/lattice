import type { WorkspaceAnchor } from "@lattice/agent-protocol";

import { getAnchorAdapterFor } from "../agent/adapters";

import { createDomGuidanceAnchor } from "./domAnchor";
import type { GuidanceAnchor } from "./types";

export function createAgentOverlayGuidanceAnchor(config: {
  id: string;
  resolveAnchor: () => WorkspaceAnchor | null;
  describe?: string;
}): GuidanceAnchor {
  const { id, resolveAnchor, describe } = config;

  return {
    id,
    isAvailable: () => {
      const anchor = resolveAnchor();
      if (!anchor) return false;
      return getAnchorAdapterFor(anchor) !== undefined;
    },
    reveal: async () => {
      const anchor = resolveAnchor();
      if (!anchor) return;
      const adapter = getAnchorAdapterFor(anchor);
      if (!adapter) return;
      await adapter.reveal(anchor, "reveal");
    },
    getRect: () => {
      const anchor = resolveAnchor();
      if (!anchor) return null;
      const adapter = getAnchorAdapterFor(anchor);
      return adapter?.getScreenRect?.(anchor) ?? null;
    },
    describe: describe ? () => describe : undefined,
  };
}

export function createAgentHighlightDomAnchor(overlayId: string, id: string): GuidanceAnchor {
  return createDomGuidanceAnchor({
    id,
    describe: `Agent overlay highlight (${overlayId})`,
    resolveElement: () =>
      document.querySelector(`[data-agent-overlay-id="${overlayId}"]`) as HTMLElement | null,
  });
}
