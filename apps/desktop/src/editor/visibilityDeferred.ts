import { useEffect, useState, type CSSProperties, type RefCallback } from "react";

/** Vertical overscan before mounting heavy embed previews (matches PDF viewer spirit). */
export const OFFSCREEN_EMBED_ROOT_MARGIN = "240px 0px";

/** Default reserved height when an image has no explicit dimensions. */
export const DEFAULT_OFFSCREEN_PLACEHOLDER_HEIGHT_PX = 120;

export interface DeferredUntilVisibleOptions {
  rootMargin?: string;
  /** When IntersectionObserver is unavailable, assume visible. */
  defaultVisible?: boolean;
}

function intersectionObserverSupported(): boolean {
  return typeof globalThis.IntersectionObserver !== "undefined";
}

export function parseEmbedDimension(value: unknown): number | null {
  if (typeof value === "number" && Number.isFinite(value) && value > 0) return value;
  if (typeof value !== "string") return null;
  const trimmed = value.trim();
  if (trimmed.endsWith("px")) {
    const parsed = Number.parseFloat(trimmed.slice(0, -2));
    return Number.isFinite(parsed) && parsed > 0 ? parsed : null;
  }
  const parsed = Number.parseFloat(trimmed);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : null;
}

export function heavyEmbedPlaceholderStyle(
  width: unknown,
  height: unknown,
  fallbackMinHeight = DEFAULT_OFFSCREEN_PLACEHOLDER_HEIGHT_PX,
): CSSProperties {
  const parsedWidth = parseEmbedDimension(width);
  const parsedHeight = parseEmbedDimension(height);
  if (parsedWidth && parsedHeight) {
    return {
      width: `${parsedWidth}px`,
      minHeight: `${parsedHeight}px`,
      aspectRatio: `${parsedWidth} / ${parsedHeight}`,
    };
  }
  if (parsedHeight) return { minHeight: `${parsedHeight}px` };
  if (parsedWidth) return { width: `${parsedWidth}px`, minHeight: `${fallbackMinHeight}px` };
  return { minHeight: `${fallbackMinHeight}px` };
}

/**
 * Defers expensive preview work until the node view is near the viewport.
 * The ProseMirror node view itself stays mounted; only `isVisible` gates I/O.
 */
export function useDeferredUntilVisible({
  rootMargin = OFFSCREEN_EMBED_ROOT_MARGIN,
  defaultVisible = !intersectionObserverSupported(),
}: DeferredUntilVisibleOptions = {}): {
  ref: RefCallback<HTMLElement>;
  isVisible: boolean;
} {
  const [element, setElement] = useState<HTMLElement | null>(null);
  const [isVisible, setIsVisible] = useState(defaultVisible);

  useEffect(() => {
    if (!element) return;
    if (!intersectionObserverSupported()) {
      setIsVisible(true);
      return;
    }

    let visible = false;
    const observer = new IntersectionObserver(
      (entries) => {
        const intersecting = entries.some((entry) => entry.isIntersecting);
        if (intersecting === visible) return;
        visible = intersecting;
        setIsVisible(intersecting);
      },
      { rootMargin, threshold: 0 },
    );
    observer.observe(element);
    return () => observer.disconnect();
  }, [element, rootMargin]);

  const ref: RefCallback<HTMLElement> = (node) => {
    setElement(node);
  };

  return { ref, isVisible };
}
