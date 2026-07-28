/**
 * Capture-only demo director helpers for Tauri Playwright.
 *
 * Injects ephemeral cursor + callout chrome into the WebView during recording
 * (not shipped UI). Used by YC / product capture to script mouse motion and
 * tooltip-style callouts over the private First Look overlay workspace.
 */
import type { TauriPage } from "@srsholmes/tauri-playwright";
import { openTreePage, scrollTreeUntilLabel } from "../perf/helpers";

export type CaptureCallout = {
  /** Stable id so multiple callouts can coexist. Defaults to "primary". */
  id?: string;
  title: string;
  body?: string;
  /** CSS selector to anchor beside; falls back to lower-left stage. */
  anchorSelector?: string;
  placement?: "top" | "bottom" | "left" | "right";
};

const CURSOR_ID = "lt-capture-cursor";
const CALLOUT_ROOT_ID = "lt-capture-callouts";
const STYLE_ID = "lt-capture-director-style";

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/** Install CSS + cursor root once per capture session. */
export async function installCaptureDirector(page: TauriPage): Promise<void> {
  await page.evaluate(`(() => {
    if (document.getElementById(${JSON.stringify(STYLE_ID)})) return;
    const style = document.createElement("style");
    style.id = ${JSON.stringify(STYLE_ID)};
    style.textContent = \`
      #${CURSOR_ID} {
        position: fixed;
        width: 18px;
        height: 18px;
        margin-left: -3px;
        margin-top: -2px;
        border-radius: 50% 50% 50% 0;
        transform: rotate(-35deg);
        background: #f4f7fb;
        border: 2px solid #a85e00;
        box-shadow: 0 6px 18px rgba(15, 23, 42, 0.35);
        pointer-events: none;
        z-index: 2147483000;
        opacity: 0;
        transition: left 420ms cubic-bezier(0.22, 1, 0.36, 1),
                    top 420ms cubic-bezier(0.22, 1, 0.36, 1),
                    opacity 180ms ease;
      }
      #${CALLOUT_ROOT_ID} {
        position: fixed;
        inset: 0;
        pointer-events: none;
        z-index: 2147482990;
      }
      .lt-capture-callout {
        position: absolute;
        max-width: min(22rem, 42vw);
        padding: 0.85rem 1rem;
        border-radius: 12px;
        border: 1px solid rgba(168, 94, 0, 0.45);
        background: rgba(15, 20, 32, 0.92);
        color: #f4f7fb;
        box-shadow: 0 18px 40px rgba(0, 0, 0, 0.35);
        font-family: "IBM Plex Sans", "Segoe UI", sans-serif;
        opacity: 0;
        transform: translateY(8px);
        transition: opacity 220ms ease, transform 220ms ease;
      }
      .lt-capture-callout.is-visible {
        opacity: 1;
        transform: translateY(0);
      }
      .lt-capture-callout__eyebrow {
        font-size: 0.7rem;
        letter-spacing: 0.16em;
        text-transform: uppercase;
        color: #d97706;
        margin-bottom: 0.35rem;
      }
      .lt-capture-callout__title {
        font-size: 1.05rem;
        font-weight: 650;
        letter-spacing: -0.01em;
        line-height: 1.25;
      }
      .lt-capture-callout__body {
        margin-top: 0.4rem;
        font-size: 0.92rem;
        line-height: 1.45;
        color: rgba(244, 247, 251, 0.82);
      }
    \`;
    document.head.appendChild(style);

    const cursor = document.createElement("div");
    cursor.id = ${JSON.stringify(CURSOR_ID)};
    document.documentElement.appendChild(cursor);

    const root = document.createElement("div");
    root.id = ${JSON.stringify(CALLOUT_ROOT_ID)};
    document.documentElement.appendChild(root);
  })()`);
}

export async function clearCaptureDirector(page: TauriPage): Promise<void> {
  await page.evaluate(`(() => {
    document.getElementById(${JSON.stringify(CURSOR_ID)})?.remove();
    document.getElementById(${JSON.stringify(CALLOUT_ROOT_ID)})?.remove();
    document.getElementById(${JSON.stringify(STYLE_ID)})?.remove();
  })()`);
}

/** Smoothly move the scripted cursor to an element center (visible in capture). */
export async function moveCaptureCursorTo(
  page: TauriPage,
  selector: string,
  options?: { settleMs?: number },
): Promise<void> {
  await page.waitForSelector(selector, options?.settleMs ?? 30_000);
  await page.evaluate(`(() => {
    const el = document.querySelector(${JSON.stringify(selector)});
    const cursor = document.getElementById(${JSON.stringify(CURSOR_ID)});
    if (!el || !cursor) return;
    const rect = el.getBoundingClientRect();
    cursor.style.opacity = "1";
    cursor.style.left = (rect.left + rect.width * 0.55) + "px";
    cursor.style.top = (rect.top + rect.height * 0.45) + "px";
  })()`);
  await sleep(options?.settleMs ?? 480);
}

export async function hideCaptureCursor(page: TauriPage): Promise<void> {
  await page.evaluate(`(() => {
    const cursor = document.getElementById(${JSON.stringify(CURSOR_ID)});
    if (cursor) cursor.style.opacity = "0";
  })()`);
}

export async function showCaptureCallout(
  page: TauriPage,
  callout: CaptureCallout,
): Promise<void> {
  const id = callout.id ?? "primary";
  const placement = callout.placement ?? "right";
  await page.evaluate(`(() => {
    const root = document.getElementById(${JSON.stringify(CALLOUT_ROOT_ID)});
    if (!root) return;
    const id = ${JSON.stringify(id)};
    let node = root.querySelector('[data-callout-id="' + id + '"]');
    if (!node) {
      node = document.createElement("aside");
      node.className = "lt-capture-callout";
      node.setAttribute("data-callout-id", id);
      root.appendChild(node);
    }
    node.innerHTML = \`
      <div class="lt-capture-callout__eyebrow">Lattice</div>
      <div class="lt-capture-callout__title"></div>
      <div class="lt-capture-callout__body" hidden></div>
    \`;
    node.querySelector(".lt-capture-callout__title").textContent = ${JSON.stringify(callout.title)};
    const body = node.querySelector(".lt-capture-callout__body");
    const bodyText = ${JSON.stringify(callout.body ?? "")};
    if (bodyText) {
      body.hidden = false;
      body.textContent = bodyText;
    } else {
      body.hidden = true;
    }

    const anchorSel = ${JSON.stringify(callout.anchorSelector ?? "")};
    const placement = ${JSON.stringify(placement)};
    const margin = 16;
    let left = margin;
    let top = window.innerHeight * 0.62;
    if (anchorSel) {
      const anchor = document.querySelector(anchorSel);
      if (anchor) {
        const rect = anchor.getBoundingClientRect();
        const w = 280;
        const h = 120;
        if (placement === "right") {
          left = Math.min(window.innerWidth - w - margin, rect.right + 14);
          top = Math.max(margin, rect.top);
        } else if (placement === "left") {
          left = Math.max(margin, rect.left - w - 14);
          top = Math.max(margin, rect.top);
        } else if (placement === "top") {
          left = Math.max(margin, rect.left);
          top = Math.max(margin, rect.top - h - 12);
        } else {
          left = Math.max(margin, rect.left);
          top = Math.min(window.innerHeight - h - margin, rect.bottom + 12);
        }
      }
    }
    node.style.left = left + "px";
    node.style.top = top + "px";
    requestAnimationFrame(() => node.classList.add("is-visible"));
  })()`);
  await sleep(220);
}

export async function hideCaptureCallouts(page: TauriPage): Promise<void> {
  await page.evaluate(`(() => {
    const root = document.getElementById(${JSON.stringify(CALLOUT_ROOT_ID)});
    if (!root) return;
    for (const node of root.querySelectorAll(".lt-capture-callout")) {
      node.classList.remove("is-visible");
    }
  })()`);
  await sleep(200);
  await page.evaluate(`(() => {
    document.getElementById(${JSON.stringify(CALLOUT_ROOT_ID)})?.replaceChildren();
  })()`);
}

/** Hover + optional callout, then open a tree resource. */
export async function scriptedOpenTreePage(
  page: TauriPage,
  label: string,
  options?: {
    callout?: CaptureCallout;
    hoverMs?: number;
    settleMs?: number;
  },
): Promise<void> {
  const selector = `[aria-label=${JSON.stringify(label)}]`;
  await scrollTreeUntilLabel(page, label);
  await moveCaptureCursorTo(page, selector, { settleMs: options?.hoverMs ?? 520 });
  await page.hover(selector);

  if (options?.callout) {
    await showCaptureCallout(page, {
      ...options.callout,
      anchorSelector: options.callout.anchorSelector ?? selector,
    });
    await sleep(options?.settleMs ?? 700);
  }

  await openTreePage(page, label);
  await sleep(options?.settleMs ?? 400);
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
