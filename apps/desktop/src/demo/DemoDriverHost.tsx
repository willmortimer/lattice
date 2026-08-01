import { useCallback, useEffect, useMemo, useState } from "react";

import {
  getAnchorRectForAriaLabel,
  queryGuidanceAnchorElementForAriaLabel,
  revealGuidanceAnchorForAriaLabel,
} from "../guidance";
import { invoke } from "../lib/ipc";
import "./demoDriver.css";

type CalloutPlacement = "top" | "bottom" | "left" | "right";

type SceneCallout = {
  title: string;
  body?: string;
  placement?: CalloutPlacement;
};

type ScenePresent = {
  enterFullscreen?: boolean;
  advanceSlides?: number;
  slideHoldMs?: number;
};

type SceneBeat = {
  id: string;
  treeLabel: string;
  chapter?: string;
  title?: string;
  caption?: string;
  requiresOverlay?: boolean;
  capture?: {
    settleMs?: number;
    holdMs?: number;
    callout?: SceneCallout;
    present?: ScenePresent;
  };
};

type SceneScript = {
  id?: string;
  title?: string;
  beats: SceneBeat[];
};

type DemoDriverConfig = {
  enabled: boolean;
  scenePath?: string | null;
  scene?: SceneScript | null;
  stageWidth?: number | null;
  stageHeight?: number | null;
};

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function scrollTreeUntilLabel(label: string): Promise<HTMLElement | null> {
  const guidanceElement = queryGuidanceAnchorElementForAriaLabel(label);
  if (guidanceElement) {
    await revealGuidanceAnchorForAriaLabel(label);
    return guidanceElement;
  }

  const selector = `[aria-label=${JSON.stringify(label)}]`;
  const list = document.querySelector(".resource-list");
  if (list instanceof HTMLElement) list.scrollTop = 0;
  for (let i = 0; i < 60; i++) {
    const el = document.querySelector(selector);
    if (el instanceof HTMLElement) {
      el.scrollIntoView({ block: "nearest" });
      return el;
    }
    if (list instanceof HTMLElement) {
      list.scrollTop += Math.max(120, list.clientHeight * 0.75);
    }
    await sleep(40);
  }
  return null;
}

async function openTreeLabel(label: string): Promise<boolean> {
  const el = await scrollTreeUntilLabel(label);
  if (!el) return false;
  el.click();
  return true;
}

function ensureCalloutRoot(): HTMLElement {
  let root = document.getElementById("lt-demo-driver-callouts");
  if (!root) {
    root = document.createElement("div");
    root.id = "lt-demo-driver-callouts";
    document.documentElement.appendChild(root);
  }
  return root;
}

function showCallout(callout: SceneCallout, anchorLabel: string): void {
  const root = ensureCalloutRoot();
  root.replaceChildren();
  const node = document.createElement("aside");
  node.className = "lt-demo-driver-callout is-visible";
  node.innerHTML = `
    <div class="lt-demo-driver-callout__eyebrow">Demo</div>
    <div class="lt-demo-driver-callout__title"></div>
    <div class="lt-demo-driver-callout__body"></div>
  `;
  node.querySelector(".lt-demo-driver-callout__title")!.textContent = callout.title;
  const body = node.querySelector(".lt-demo-driver-callout__body") as HTMLElement;
  if (callout.body) body.textContent = callout.body;
  else body.hidden = true;
  root.appendChild(node);

  const anchorRect = getAnchorRectForAriaLabel(anchorLabel);
  const margin = 16;
  let left = margin;
  let top = window.innerHeight * 0.58;
  if (anchorRect) {
    const rect = anchorRect;
    const placement = callout.placement ?? "right";
    if (placement === "right") {
      left = Math.min(window.innerWidth - 300, rect.right + 14);
      top = Math.max(margin, rect.top);
    } else if (placement === "left") {
      left = Math.max(margin, rect.left - 300);
      top = Math.max(margin, rect.top);
    } else if (placement === "top") {
      left = Math.max(margin, rect.left);
      top = Math.max(margin, rect.top - 120);
    } else {
      left = Math.max(margin, rect.left);
      top = Math.min(window.innerHeight - 140, rect.bottom + 12);
    }
  }
  node.style.left = `${left}px`;
  node.style.top = `${top}px`;
}

function clearCallout(): void {
  document.getElementById("lt-demo-driver-callouts")?.replaceChildren();
}

async function presentDeck(present: ScenePresent): Promise<void> {
  if (present.enterFullscreen) {
    const button = Array.from(document.querySelectorAll("button")).find(
      (el) => el.textContent?.trim() === "Fullscreen",
    );
    button?.click();
    await sleep(500);
  }
  const slides = Math.max(0, present.advanceSlides ?? 0);
  const hold = present.slideHoldMs ?? 1600;
  for (let i = 0; i < slides; i++) {
    window.dispatchEvent(
      new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true }),
    );
    await sleep(hold);
  }
}

/**
 * Floating control surface for `LATTICE_DEMO_DRIVER=1` (exec-for-dev only).
 * Automates the shared scene script via DOM — same selectors as YC capture.
 */
export function DemoDriverHost() {
  const [config, setConfig] = useState<DemoDriverConfig | null>(null);
  const [running, setRunning] = useState(false);
  const [status, setStatus] = useState("Idle");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const next = await invoke<DemoDriverConfig>("get_demo_driver_config");
        if (cancelled) return;
        setConfig(next);
        if (next.enabled) {
          await invoke("apply_demo_stage");
        }
      } catch {
        if (!cancelled) setConfig({ enabled: false });
      }
    })();
    return () => {
      cancelled = true;
      clearCallout();
    };
  }, []);

  const beats = useMemo(() => config?.scene?.beats ?? [], [config]);

  const run = useCallback(async () => {
    if (!config?.enabled || running) return;
    setRunning(true);
    setError(null);
    try {
      await invoke("apply_demo_stage");
      for (const beat of beats) {
        if (beat.requiresOverlay) {
          const ready = await scrollTreeUntilLabel(beat.treeLabel);
          if (!ready) {
            setStatus(`Skip ${beat.id} (overlay missing)`);
            continue;
          }
        }
        setStatus(`Open ${beat.id}`);
        const opened = await openTreeLabel(beat.treeLabel);
        if (!opened) {
          setStatus(`Missing ${beat.treeLabel}`);
          continue;
        }
        const callout = beat.capture?.callout;
        if (callout) showCallout(callout, beat.treeLabel);
        await sleep(beat.capture?.settleMs ?? 900);
        if (beat.capture?.present) {
          setStatus(`Present ${beat.id}`);
          await presentDeck(beat.capture.present);
        } else {
          await sleep(beat.capture?.holdMs ?? 1800);
        }
        clearCallout();
      }
      setStatus("Done");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setStatus("Failed");
    } finally {
      clearCallout();
      setRunning(false);
    }
  }, [beats, config?.enabled, running]);

  useEffect(() => {
    if (!config?.enabled) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.metaKey && event.shiftKey && event.key.toLowerCase() === "d") {
        event.preventDefault();
        void run();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [config?.enabled, run]);

  if (!config?.enabled) return null;

  return (
    <div className="lt-demo-driver" role="complementary" aria-label="Demo driver">
      <div className="lt-demo-driver__title">Demo driver</div>
      <p className="lt-demo-driver__meta">
        {config.scene?.title ?? "YC scene"} · ⌘⇧D
      </p>
      <button
        type="button"
        className="lt-demo-driver__run"
        disabled={running || beats.length === 0}
        onClick={() => void run()}
      >
        {running ? "Running…" : "Play scene"}
      </button>
      <p className="lt-demo-driver__status">{status}</p>
      {error ? <p className="lt-demo-driver__error">{error}</p> : null}
    </div>
  );
}
