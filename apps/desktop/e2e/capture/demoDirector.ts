/**
 * Capture-only helpers for Tauri Playwright product capture.
 *
 * Cursor / callout chrome is NOT baked into footage. Capture records clean UI
 * plus an event track (rects + timings) for Remotion overlays.
 */
import type { TauriPage } from "@srsholmes/tauri-playwright";
import { openTreePage, scrollTreeUntilLabel } from "../perf/helpers";

export type CaptureCallout = {
  id?: string;
  title: string;
  body?: string;
  anchorSelector?: string;
  placement?: "top" | "bottom" | "left" | "right";
};

export type CaptureRect = {
  x: number;
  y: number;
  width: number;
  height: number;
};

export type CaptureEvent =
  | {
      type: "cursor.move";
      atMs: number;
      target?: string;
      rect: CaptureRect;
    }
  | {
      type: "click";
      atMs: number;
      target?: string;
      rect?: CaptureRect;
    }
  | {
      type: "callout";
      atMs: number;
      durationMs: number;
      copyId?: string;
      callout: CaptureCallout;
      rect?: CaptureRect;
    };

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

export async function measureSelectorRect(
  page: TauriPage,
  selector: string,
): Promise<CaptureRect | null> {
  const rect = await page.evaluate(`(() => {
    const el = document.querySelector(${JSON.stringify(selector)});
    if (!el) return null;
    const r = el.getBoundingClientRect();
    return { x: r.x, y: r.y, width: r.width, height: r.height };
  })()`);
  return (rect as CaptureRect | null) ?? null;
}

/** Hover + open a tree resource; emit overlay events instead of baking UI. */
export async function scriptedOpenTreePage(
  page: TauriPage,
  label: string,
  options?: {
    callout?: CaptureCallout;
    hoverMs?: number;
    settleMs?: number;
    /** Epoch ms when recording started (for event atMs). */
    recordingStartedAtMs?: number;
  },
): Promise<CaptureEvent[]> {
  const events: CaptureEvent[] = [];
  const started = options?.recordingStartedAtMs ?? Date.now();
  const at = () => Math.max(0, Date.now() - started);

  const selector = `[aria-label=${JSON.stringify(label)}]`;
  await scrollTreeUntilLabel(page, label);
  await page.waitForSelector(selector, 30_000);

  const hoverMs = options?.hoverMs ?? 520;
  const rect = await measureSelectorRect(page, selector);
  if (rect) {
    events.push({
      type: "cursor.move",
      atMs: at(),
      target: label,
      rect,
    });
  }
  await page.hover(selector);
  await sleep(hoverMs);

  if (options?.callout) {
    const calloutAt = at();
    const settleMs = options.settleMs ?? 700;
    events.push({
      type: "callout",
      atMs: calloutAt,
      durationMs: settleMs + 900,
      copyId: options.callout.id ?? label,
      callout: {
        ...options.callout,
        anchorSelector: options.callout.anchorSelector ?? selector,
      },
      rect: rect ?? undefined,
    });
    await sleep(settleMs);
  }

  const clickRect = (await measureSelectorRect(page, selector)) ?? rect;
  events.push({
    type: "click",
    atMs: at(),
    target: label,
    rect: clickRect ?? undefined,
  });
  await openTreePage(page, label);
  await sleep(options?.settleMs ?? 400);
  return events;
}

/** True when a private overlay resource is visible in the tree (after seed merge). */
export async function waitForOverlayTreeLabel(
  page: TauriPage,
  label: string,
  timeoutMs = 45_000,
): Promise<boolean> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    await scrollTreeUntilLabel(page, label);
    const visible = await page.isVisible(`[aria-label=${JSON.stringify(label)}]`);
    if (visible) return true;
    await sleep(400);
  }
  return false;
}
