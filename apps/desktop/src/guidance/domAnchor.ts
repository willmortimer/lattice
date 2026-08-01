import type { GuidanceAnchor } from "./types";

export const GUIDANCE_ANCHOR_ATTR = "data-guidance-anchor";

export function queryGuidanceAnchorElement(id: string): HTMLElement | null {
  return document.querySelector(`[${GUIDANCE_ANCHOR_ATTR}="${id}"]`);
}

export function elementRect(element: HTMLElement | null): DOMRect | null {
  if (!element) return null;
  const rect = element.getBoundingClientRect();
  if (rect.width === 0 && rect.height === 0) return null;
  return rect;
}

function scrollIntoViewIfNeeded(element: HTMLElement): void {
  element.scrollIntoView({ block: "nearest", inline: "nearest" });
}

export function createDomGuidanceAnchor(config: {
  id: string;
  describe?: string;
  resolveElement?: () => HTMLElement | null;
  reveal?: () => Promise<void>;
  focus?: () => void;
}): GuidanceAnchor {
  const resolve = config.resolveElement ?? (() => queryGuidanceAnchorElement(config.id));
  const describe = config.describe;

  return {
    id: config.id,
    isAvailable: () => resolve() !== null,
    reveal: async () => {
      const element = resolve();
      if (!element) return;
      if (config.reveal) {
        await config.reveal();
        return;
      }
      scrollIntoViewIfNeeded(element);
    },
    getRect: () => elementRect(resolve()),
    focus:
      config.focus ??
      (() => {
        resolve()?.focus();
      }),
    describe: describe ? () => describe : undefined,
  };
}
