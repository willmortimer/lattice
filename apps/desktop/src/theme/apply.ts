/** Theme apply + persistence for the desktop shell. */

export const THEME_MIRROR_KEY = "lattice.theme.mirror";

export interface ThemeMirror {
  background: string;
  appearance: "dark" | "light" | string;
  vars: Record<string, string>;
  id: string;
  updatedAt: number;
}

export interface ResolvedThemePayload {
  id: string;
  name: string;
  appearance: string;
  sourcePath: string;
  vars: Record<string, string>;
  background: string;
  settings: {
    mode: "fixed" | "auto" | string;
    theme: string;
    pair: { dark: string; light: string };
  };
  workspaceOverride: { theme?: string | null; accent?: string | null };
  diagnostics: Array<{ path: string; message: string }>;
}

export interface ThemeSummaryPayload {
  id: string;
  name: string;
  appearance: string;
  source: "builtin" | "user" | string;
  path: string;
}

export interface ThemeCatalogPayload {
  themes: ThemeSummaryPayload[];
  diagnostics: Array<{ path: string; message: string }>;
  resolved: ResolvedThemePayload;
}

/** Detect OS / browser color-scheme preference. */
export function detectSystemAppearance(): "dark" | "light" {
  if (typeof window === "undefined" || !window.matchMedia) return "dark";
  return window.matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark";
}

/** Apply CSS variables to `:root` and persist a first-paint mirror. */
export function applyResolvedTheme(resolved: ResolvedThemePayload): void {
  const root = document.documentElement;
  root.style.colorScheme = resolved.appearance === "light" ? "light" : "dark";
  root.style.background = resolved.background;

  // Optional vars (e.g. --lt-term-* from a theme's terminal block) must not
  // leak from the previous theme when the new one omits them.
  for (let i = root.style.length - 1; i >= 0; i--) {
    const name = root.style[i];
    if (name.startsWith("--lt-term-") && !(name in resolved.vars)) {
      root.style.removeProperty(name);
    }
  }

  for (const [key, value] of Object.entries(resolved.vars)) {
    root.style.setProperty(key, value);
  }

  persistThemeMirror({
    background: resolved.background,
    appearance: resolved.appearance,
    vars: resolved.vars,
    id: resolved.id,
    updatedAt: Date.now(),
  });

  void syncNativeWindowBackground(resolved.background);
}

export function persistThemeMirror(mirror: ThemeMirror): void {
  try {
    localStorage.setItem(THEME_MIRROR_KEY, JSON.stringify(mirror));
  } catch {
    // Quota / private mode — first paint may flash; runtime still works.
  }
}

export function readThemeMirror(): ThemeMirror | null {
  try {
    const raw = localStorage.getItem(THEME_MIRROR_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as ThemeMirror;
    if (!parsed?.background || !parsed?.vars) return null;
    return parsed;
  } catch {
    return null;
  }
}

async function syncNativeWindowBackground(hex: string): Promise<void> {
  if (!("__TAURI_INTERNALS__" in window)) return;
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().setBackgroundColor(hex);
  } catch {
    // Older webview / capability missing — CSS ground still paints.
  }
}
