import {
  resolveSettingsDeepLink,
  type SettingsDeepLinkTarget,
} from "./settingsCatalog";

export type { SettingsDeepLinkTarget };

const SETTINGS_DEEPLINK_EVENT = "lattice:settings-deeplink";

/** Extract the settings path from a lattice URL or hash fragment. */
export function parseSettingsDeepLinkUrl(url: string): string | null {
  const trimmed = url.trim();
  if (!trimmed) return null;

  if (trimmed.startsWith("#")) {
    const hash = trimmed.replace(/^#\/?/, "");
    if (hash === "settings" || hash.startsWith("settings/")) {
      return hash.replace(/^settings\/?/, "") || null;
    }
    return null;
  }

  try {
    const parsed = new URL(trimmed);
    if (parsed.protocol !== "lattice:") return null;
    if (parsed.hostname === "settings") {
      return parsed.pathname.replace(/^\/+/, "") || null;
    }
    const path = parsed.pathname.replace(/^\/+/, "");
    if (path === "settings" || path.startsWith("settings/")) {
      return path.replace(/^settings\/?/, "") || null;
    }
  } catch {
    return null;
  }

  return null;
}

/** Navigate settings to a catalog section/row. Returns false when the path is unknown. */
export function openSettingsDeepLink(path: string): boolean {
  const target = resolveSettingsDeepLink(path);
  if (!target) return false;
  window.dispatchEvent(
    new CustomEvent<SettingsDeepLinkTarget>(SETTINGS_DEEPLINK_EVENT, { detail: target }),
  );
  return true;
}

/** Open settings from a full lattice://settings/… URL or hash fragment. */
export function openSettingsDeepLinkUrl(url: string): boolean {
  const path = parseSettingsDeepLinkUrl(url);
  if (path === null) return false;
  return openSettingsDeepLink(path);
}

export function subscribeSettingsDeepLink(
  listener: (target: SettingsDeepLinkTarget) => void,
): () => void {
  const handler = (event: Event) => {
    listener((event as CustomEvent<SettingsDeepLinkTarget>).detail);
  };
  window.addEventListener(SETTINGS_DEEPLINK_EVENT, handler);
  return () => window.removeEventListener(SETTINGS_DEEPLINK_EVENT, handler);
}
