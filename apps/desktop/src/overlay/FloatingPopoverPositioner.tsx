import type { Placement } from "@floating-ui/react-dom";
import type { CSSProperties, ReactNode } from "react";
import { createPortal } from "react-dom";

import { useVirtualFloatingPosition } from "./useVirtualFloatingPosition";
import type { RectSource } from "./floatingPosition";

type FloatingPopoverPositionerProps = {
  rect: RectSource;
  placement?: Placement;
  sideOffset?: number;
  padding?: number;
  enabled?: boolean;
  className?: string;
  style?: CSSProperties;
  children: ReactNode;
  /** Portal to `document.body` so overflow clipping cannot hide the callout. */
  portal?: boolean;
};

/**
 * Positions callout content against a virtual rect (not a DOM trigger).
 * Do not nest Base UI `Popover.Popup` here — that requires `<Popover.Positioner>`.
 */
export function FloatingPopoverPositioner({
  rect,
  placement = "bottom",
  sideOffset,
  padding,
  enabled = true,
  className,
  style,
  children,
  portal = true,
}: FloatingPopoverPositionerProps) {
  const { refs, floatingStyles } = useVirtualFloatingPosition({
    rect,
    placement,
    sideOffset,
    padding,
    enabled,
    mode: "callout",
  });

  if (!enabled) return null;

  const node = (
    <div
      ref={refs.setFloating}
      className={className}
      style={{ ...floatingStyles, ...style }}
    >
      {children}
    </div>
  );

  if (portal && typeof document !== "undefined") {
    return createPortal(node, document.body);
  }
  return node;
}
