import {
  flip,
  offset,
  shift,
  type Middleware,
  type Placement,
  type VirtualElement,
} from "@floating-ui/react-dom";

export const EMPTY_DOM_RECT = new DOMRect(0, 0, 0, 0);

export type RectSource = DOMRect | null | (() => DOMRect | null);

export function resolveRect(source: RectSource): DOMRect | null {
  return typeof source === "function" ? source() : source;
}

export function createVirtualElementFromRect(source: RectSource): VirtualElement {
  return {
    getBoundingClientRect: () => resolveRect(source) ?? EMPTY_DOM_RECT,
  };
}

export function inflateRect(rect: DOMRect, padding: number): DOMRect {
  return new DOMRect(
    rect.x - padding,
    rect.y - padding,
    rect.width + padding * 2,
    rect.height + padding * 2,
  );
}

export type TourSide = "top" | "bottom" | "left" | "right";

export function tourSideToPlacement(side: TourSide): Placement {
  switch (side) {
    case "top":
      return "top";
    case "bottom":
      return "bottom";
    case "left":
      return "left";
    case "right":
      return "right";
    default: {
      const _exhaustive: never = side;
      return _exhaustive;
    }
  }
}

export type OverlayFloatingOptions = {
  sideOffset?: number;
  padding?: number;
};

export function createCalloutFloatingMiddleware(
  options: OverlayFloatingOptions = {},
): Middleware[] {
  const { sideOffset = 12, padding = 8 } = options;
  return [offset(sideOffset), flip({ padding }), shift({ padding })];
}

/** Pins the floating element to the reference box (spotlight frame). */
export function createSpotlightMatchMiddleware(): Middleware {
  return {
    name: "matchReference",
    fn({ rects }) {
      return {
        x: rects.reference.x,
        y: rects.reference.y,
        data: {
          width: rects.reference.width,
          height: rects.reference.height,
        },
      };
    },
  };
}
