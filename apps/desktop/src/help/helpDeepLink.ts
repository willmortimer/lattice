const HELP_DEEPLINK_EVENT = "lattice:help-deeplink";

/** Extract the page stem from a lattice help URL or hash fragment. */
export function parseHelpDeepLinkUrl(url: string): string | null {
  const trimmed = url.trim();
  if (!trimmed) return null;

  if (trimmed.startsWith("#")) {
    const hash = trimmed.replace(/^#\/?/, "");
    if (hash === "help" || hash.startsWith("help/")) {
      const stem = hash.replace(/^help\/?/, "").trim();
      return stem || "welcome";
    }
    return null;
  }

  try {
    const parsed = new URL(trimmed);
    if (parsed.protocol !== "lattice:") return null;
    if (parsed.hostname === "help") {
      const pathStem = parsed.pathname.replace(/^\/+/, "").trim();
      return pathStem || "welcome";
    }
    const path = parsed.pathname.replace(/^\/+/, "");
    if (path === "help" || path.startsWith("help/")) {
      const stem = path.replace(/^help\/?/, "").trim();
      return stem || "welcome";
    }
  } catch {
    return null;
  }

  return null;
}

/** Open Help on a page stem (dispatches a window event for the panel). */
export function openHelpDeepLink(stem: string): boolean {
  const normalized = stem.trim();
  if (!normalized) return false;
  window.dispatchEvent(
    new CustomEvent<string>(HELP_DEEPLINK_EVENT, { detail: normalized }),
  );
  return true;
}

/** Open Help from a full `lattice://help/…` URL or `#help/…` hash. */
export function openHelpDeepLinkUrl(url: string): boolean {
  const stem = parseHelpDeepLinkUrl(url);
  if (!stem) return false;
  return openHelpDeepLink(stem);
}

export function subscribeHelpDeepLink(listener: (stem: string) => void): () => void {
  const handler = (event: Event) => {
    const stem = (event as CustomEvent<string>).detail;
    if (typeof stem === "string" && stem.trim()) {
      listener(stem.trim());
    }
  };
  window.addEventListener(HELP_DEEPLINK_EVENT, handler);
  return () => window.removeEventListener(HELP_DEEPLINK_EVENT, handler);
}
