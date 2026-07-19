/** Theme apply + persistence for the desktop shell. */

export const THEME_MIRROR_KEY = "lattice.theme.mirror";

export interface ThemeMirrorEntry {
  background: string;
  appearance: "dark" | "light" | string;
  vars: Record<string, string>;
  id: string;
}

export interface ThemeMirror extends ThemeMirrorEntry {
  updatedAt: number;
  /** `auto` picks `byAppearance` from prefers-color-scheme on first paint. */
  mode?: "fixed" | "auto" | string;
  /** Last-resolved theme for each system appearance (auto-mode first paint). */
  byAppearance?: Partial<Record<"dark" | "light", ThemeMirrorEntry>>;
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

function appearanceKey(appearance: string): "dark" | "light" {
  return appearance === "light" ? "light" : "dark";
}

/** Pick the first-paint entry for the current (or given) system appearance. */
export function selectThemeMirrorEntry(
  mirror: ThemeMirror,
  system: "dark" | "light" = detectSystemAppearance(),
): ThemeMirrorEntry {
  if (mirror.mode === "auto" && mirror.byAppearance?.[system]) {
    return mirror.byAppearance[system]!;
  }
  return {
    background: mirror.background,
    appearance: mirror.appearance,
    vars: mirror.vars,
    id: mirror.id,
  };
}

/** Build the persisted first-paint mirror from a resolved theme. */
export function buildThemeMirror(
  resolved: ResolvedThemePayload,
  previous: ThemeMirror | null,
  updatedAt: number = Date.now(),
): ThemeMirror {
  const entry: ThemeMirrorEntry = {
    background: resolved.background,
    appearance: resolved.appearance,
    vars: resolved.vars,
    id: resolved.id,
  };
  return {
    ...entry,
    updatedAt,
    mode: resolved.settings.mode === "auto" ? "auto" : "fixed",
    byAppearance: {
      ...previous?.byAppearance,
      [appearanceKey(resolved.appearance)]: entry,
    },
  };
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

  persistThemeMirror(buildThemeMirror(resolved, readThemeMirror()));
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
