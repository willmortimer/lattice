import { useEffect, useRef } from "react";

import { useAgentSessionStore } from "./agentStore";
import {
  applyActiveOverlays,
  clearOverlayHighlights,
  type OverlayClearFn,
} from "./agentOverlayEffects";

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

  return null;
}
