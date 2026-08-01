import type { Placement } from "@floating-ui/react-dom";
import type { CSSProperties, ReactNode } from "react";

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
};

export function FloatingPopoverPositioner({
  rect,
  placement = "bottom",
  sideOffset,
  padding,
  enabled = true,
  className,
  style,
  children,
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

  return (
    <div
      ref={refs.setFloating}
      className={className}
      style={{ ...floatingStyles, ...style }}
    >
      {children}
    </div>
  );
}
