import { GUIDANCE_ANCHOR_ATTR, queryGuidanceAnchorElement } from "./domAnchor";
import { getGuidanceAnchor } from "./registry";
import type { GuidanceAnchor } from "./types";

/** Known ARIA labels that map to registered guidance anchors. */
const ARIA_LABEL_TO_ANCHOR_ID: Record<string, string> = {
  "Create resource": "resource-tree.new-page",
  "Show agent": "agent.panel.toggle",
  "Hide agent": "agent.panel.toggle",
  "Workspace menu": "shell.workspace-switcher",
};

export function resolveGuidanceAnchorIdFromAriaLabel(label: string): string | null {
  const mapped = ARIA_LABEL_TO_ANCHOR_ID[label];
  if (mapped) return mapped;

  const selector = `[aria-label=${JSON.stringify(label)}][${GUIDANCE_ANCHOR_ATTR}]`;
  const element = document.querySelector(selector);
  if (element instanceof HTMLElement) {
    return element.getAttribute(GUIDANCE_ANCHOR_ATTR);
  }
  return null;
}

export function resolveGuidanceAnchorFromAriaLabel(label: string): GuidanceAnchor | undefined {
  const anchorId = resolveGuidanceAnchorIdFromAriaLabel(label);
  if (!anchorId) return undefined;
  return getGuidanceAnchor(anchorId);
}

export function getAnchorRectForAriaLabel(label: string): DOMRect | null {
  const anchor = resolveGuidanceAnchorFromAriaLabel(label);
  if (anchor?.isAvailable()) {
    return anchor.getRect();
  }

  const element = document.querySelector(`[aria-label=${JSON.stringify(label)}]`);
  if (element instanceof HTMLElement) {
    const rect = element.getBoundingClientRect();
    if (rect.width === 0 && rect.height === 0) return null;
    return rect;
  }
  return null;
}

export async function revealGuidanceAnchorForAriaLabel(label: string): Promise<GuidanceAnchor | null> {
  const anchor = resolveGuidanceAnchorFromAriaLabel(label);
  if (!anchor) return null;
  await anchor.reveal();
  return anchor;
}

export function queryGuidanceAnchorElementForAriaLabel(label: string): HTMLElement | null {
  const anchorId = resolveGuidanceAnchorIdFromAriaLabel(label);
  if (anchorId) {
    return queryGuidanceAnchorElement(anchorId);
  }
  const element = document.querySelector(`[aria-label=${JSON.stringify(label)}]`);
  return element instanceof HTMLElement ? element : null;
}
