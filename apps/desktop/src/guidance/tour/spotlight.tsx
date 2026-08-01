import type { CSSProperties } from "react";

import "./spotlight.css";

const SPOTLIGHT_PADDING = 6;

type GuidanceSpotlightProps = {
  rect: DOMRect | null;
};

export function GuidanceSpotlight({ rect }: GuidanceSpotlightProps) {
  if (!rect) return null;

  const frameStyle: CSSProperties = {
    top: rect.top - SPOTLIGHT_PADDING,
    left: rect.left - SPOTLIGHT_PADDING,
    width: rect.width + SPOTLIGHT_PADDING * 2,
    height: rect.height + SPOTLIGHT_PADDING * 2,
  };

  return (
    <div className="guidance-spotlight" aria-hidden="true">
      <div className="guidance-spotlight__frame" style={frameStyle} />
    </div>
  );
}
