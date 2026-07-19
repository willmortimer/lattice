import type { MouseEvent as ReactMouseEvent } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";

import { hasTauri } from "../lib/ipc";

/**
 * How an editor `<a href>` should be handled on click.
 *
 * Workspace targets (wiki + relative Markdown) navigate inside Lattice.
 * External http(s)/mailto/tel open in the system browser. Fragments stay
 * in-document. Unknown schemes are ignored so we never invent navigation.
 */
export type ClassifiedEditorHref =
  | { kind: "workspace"; target: string }
  | { kind: "external"; url: string }
  | { kind: "fragment"; hash: string }
  | { kind: "ignored" };

const EXTERNAL_SCHEME = /^(https?:|mailto:|tel:)/i;
const ANY_SCHEME = /^[a-z][a-z0-9+.-]*:/i;

export function classifyEditorHref(href: string): ClassifiedEditorHref {
  const trimmed = href.trim();
  if (!trimmed || trimmed === "#") return { kind: "ignored" };

  if (trimmed.startsWith("wiki:")) {
    return {
      kind: "workspace",
      target: decodeURIComponent(trimmed.slice("wiki:".length)),
    };
  }

  if (trimmed.startsWith("#")) {
    return { kind: "fragment", hash: trimmed.slice(1) };
  }

  if (EXTERNAL_SCHEME.test(trimmed)) {
    return { kind: "external", url: trimmed };
  }

  // javascript:, data:, asset:, tauri:, etc. — do not treat as workspace paths.
  if (ANY_SCHEME.test(trimmed)) {
    return { kind: "ignored" };
  }

  return { kind: "workspace", target: decodeURIComponent(trimmed) };
}

async function openExternalHref(url: string): Promise<void> {
  if (!hasTauri) {
    window.open(url, "_blank", "noopener,noreferrer");
    return;
  }
  await openUrl(url);
}

/**
 * Shared click routing for page edit/preview (and notebook markdown cells).
 * Returns true when the event was handled so callers can skip further work.
 */
export function handleEditorLinkClick(
  event: ReactMouseEvent | MouseEvent,
  onOpenWorkspaceLink?: (target: string) => void,
): boolean {
  if (event.defaultPrevented || event.button !== 0) return false;
  if (event.metaKey || event.altKey || event.ctrlKey || event.shiftKey) return false;

  const anchor = (event.target as HTMLElement | null)?.closest?.("a");
  if (!anchor) return false;
  const href = anchor.getAttribute("href");
  if (!href) return false;

  const classified = classifyEditorHref(href);
  switch (classified.kind) {
    case "workspace": {
      if (!onOpenWorkspaceLink) return false;
      event.preventDefault();
      event.stopPropagation();
      onOpenWorkspaceLink(classified.target);
      return true;
    }
    case "external": {
      event.preventDefault();
      event.stopPropagation();
      void openExternalHref(classified.url);
      return true;
    }
    case "fragment":
    case "ignored":
      return false;
    default: {
      const unreachable: never = classified;
      return unreachable;
    }
  }
}
