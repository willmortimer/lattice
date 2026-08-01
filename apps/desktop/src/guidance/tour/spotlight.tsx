import { useMemo } from "react";

import { inflateRect } from "../../overlay/floatingPosition";
import { useVirtualFloatingPosition } from "../../overlay/useVirtualFloatingPosition";

import "./spotlight.css";

const SPOTLIGHT_PADDING = 6;

type GuidanceSpotlightProps = {
  rect: DOMRect | null;
};

export function GuidanceSpotlight({ rect }: GuidanceSpotlightProps) {
  const paddedRect = useMemo(
    () => (rect ? inflateRect(rect, SPOTLIGHT_PADDING) : null),
    [rect],
  );

  const { refs, floatingStyles } = useVirtualFloatingPosition({
    rect: paddedRect,
    enabled: paddedRect !== null,
    mode: "spotlight",
  });

  if (!paddedRect) return null;

  return (
    <div className="guidance-spotlight" aria-hidden="true">
      <div
        ref={refs.setFloating}
        className="guidance-spotlight__frame"
        style={{
          ...floatingStyles,
          width: paddedRect.width,
          height: paddedRect.height,
        }}
      />
    </div>
  );
}
