import {
  autoUpdate,
  useFloating,
  type Placement,
  type VirtualElement,
} from "@floating-ui/react-dom";
import { useMemo } from "react";

import {
  createCalloutFloatingMiddleware,
  createSpotlightMatchMiddleware,
  createVirtualElementFromRect,
  type OverlayFloatingOptions,
  type RectSource,
} from "./floatingPosition";

export type VirtualFloatingMode = "callout" | "spotlight";

export type UseVirtualFloatingPositionOptions = OverlayFloatingOptions & {
  rect: RectSource;
  placement?: Placement;
  enabled?: boolean;
  mode?: VirtualFloatingMode;
};

export function useVirtualFloatingPosition({
  rect,
  placement = "bottom",
  enabled = true,
  mode = "callout",
  sideOffset,
  padding,
}: UseVirtualFloatingPositionOptions) {
  const reference = useMemo<VirtualElement>(
    () => createVirtualElementFromRect(rect),
    [rect],
  );

  const middleware = useMemo(
    () =>
      mode === "spotlight"
        ? [createSpotlightMatchMiddleware()]
        : createCalloutFloatingMiddleware({ sideOffset, padding }),
    [mode, sideOffset, padding],
  );

  return useFloating({
    placement,
    strategy: "fixed",
    middleware,
    elements: { reference },
    whileElementsMounted: enabled ? autoUpdate : undefined,
  });
}
