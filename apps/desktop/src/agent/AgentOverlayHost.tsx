import "./agentOverlayHost.css";

import { PopoverPopup } from "@lattice/ui";
import { useCallback, useEffect, useRef } from "react";

import { useAgentSessionStore } from "./agentStore";
import type { ActiveOverlay } from "./agentStore";
import {
  applyActiveOverlays,
  clearOverlayHighlights,
  type OverlayClearFn,
} from "./agentOverlayEffects";
import { FloatingPopoverPositioner } from "../overlay/FloatingPopoverPositioner";
import { resolveOverlayAnchorRect } from "../overlay/resolveOverlayAnchorRect";

type AgentOverlayCalloutProps = {
  overlay: ActiveOverlay;
};

function AgentOverlayCallout({ overlay }: AgentOverlayCalloutProps) {
  const getRect = useCallback(() => resolveOverlayAnchorRect(overlay), [overlay]);

  if (!overlay.commentary) return null;

  return (
    <FloatingPopoverPositioner
      rect={getRect}
      placement="bottom"
      sideOffset={12}
      className="agent-overlay-callout"
      style={{ zIndex: 1201 }}
    >
      <PopoverPopup className="agent-overlay-callout__popup" role="note">
        <div className="agent-overlay-callout__eyebrow">Agent</div>
        <p className="agent-overlay-callout__body">{overlay.commentary}</p>
      </PopoverPopup>
    </FloatingPopoverPositioner>
  );
}

export function AgentOverlayHost() {
  const activeOverlays = useAgentSessionStore((state) => state.activeOverlays);
  const followMode = useAgentSessionStore((state) => state.followMode);
  const clearsRef = useRef<OverlayClearFn[]>([]);

  useEffect(() => {
    clearOverlayHighlights(clearsRef.current);
    clearsRef.current = applyActiveOverlays(activeOverlays, followMode);

    return () => {
      clearOverlayHighlights(clearsRef.current);
      clearsRef.current = [];
    };
  }, [activeOverlays, followMode]);

  const overlays = Object.values(activeOverlays);

  return (
    <>
      {overlays.map((overlay) =>
        overlay.commentary ? (
          <AgentOverlayCallout key={overlay.overlayId} overlay={overlay} />
        ) : null,
      )}
    </>
  );
}
