/**
 * Browser-demo theme catalog — builtins only, prefs in localStorage.
 * Mirrors Tauri `list_themes` / `set_theme` enough for palette UX.
 */

import slateCss from "../theme-tokens.css?raw";

import {
  detectSystemAppearance,
  type ResolvedThemePayload,
  type ThemeCatalogPayload,
} from "./apply";

const DEMO_PREFS_KEY = "lattice.theme.demoPrefs";

interface DemoPrefs {
  mode: "fixed" | "auto";
  theme: string;
  pair: { dark: string; light: string };
}

const DEFAULT_PREFS: DemoPrefs = {
  mode: "fixed",
  theme: "lattice-slate",
  pair: { dark: "lattice-slate", light: "lattice-paper" },
};

/** Paper light tokens — kept in sync with themes/lattice-paper.theme.yaml roles. */
const PAPER_VARS: Record<string, string> = {
  "--lt-bg": "#f4f6fa",
  "--lt-bg-raise": "#eef1f7",
  "--lt-panel": "#ffffff",
  "--lt-slate": "#5a6b86",
  "--lt-text": "#121822",
  "--lt-text-soft": "#3a4558",
  "--lt-muted": "#6b778c",
  "--lt-faint": "#8b96a8",
  "--lt-accent": "#c47a0a",
  "--lt-accent-bright": "#a86408",
  "--lt-accent-deep": "#8a5206",
  "--lt-danger": "#b42318",
  "--lt-shadow": "#0a0d13",
  "--lt-on-accent": "#fff8ef",
  "--lt-surface": "var(--lt-panel)",
  "--lt-hover": "color-mix(in oklch, var(--lt-slate) 7%, transparent)",
  "--lt-line": "color-mix(in oklch, var(--lt-slate) 12%, transparent)",
  "--lt-line-strong": "color-mix(in oklch, var(--lt-slate) 22%, transparent)",
  "--lt-border": "color-mix(in oklch, var(--lt-slate) 18%, transparent)",
  "--lt-accent-wash": "color-mix(in oklch, var(--lt-accent) 10%, transparent)",
  "--lt-accent-glow": "color-mix(in oklch, var(--lt-accent) 55%, transparent)",
  "--lt-accent-glow-soft": "color-mix(in oklch, var(--lt-accent) 35%, transparent)",
  "--lt-accent-glow-mid": "color-mix(in oklch, var(--lt-accent) 22%, transparent)",
  "--lt-accent-glow-strong": "color-mix(in oklch, var(--lt-accent) 45%, transparent)",
  "--lt-accent-underline": "color-mix(in oklch, var(--lt-accent) 35%, transparent)",
  "--lt-node-dot": "color-mix(in oklch, var(--lt-slate) 26%, transparent)",
  "--lt-node-dot-soft": "color-mix(in oklch, var(--lt-slate) 20%, transparent)",
  "--lt-scrim": "color-mix(in oklch, var(--lt-bg) 60%, transparent)",
  "--lt-scrim-deep": "color-mix(in oklch, var(--lt-bg) 72%, #06080c)",
  "--lt-shadow-md": "color-mix(in oklch, var(--lt-shadow) 35%, transparent)",
  "--lt-shadow-lg": "color-mix(in oklch, var(--lt-shadow) 45%, transparent)",
  "--lt-font-display": '"Fraunces Variable", "Fraunces", Georgia, serif',
  "--lt-font-ui": '"Space Grotesk Variable", "Space Grotesk", system-ui, sans-serif',
  "--lt-font-mono":
    '"JetBrains Mono Variable", "JetBrains Mono", ui-monospace, "SF Mono", Menlo, monospace',
  "--lt-radius": "9px",
  "--lt-radius-sm": "6px",
  "--lt-radius-lg": "14px",
  "--lt-grid": "34px",
  "--lt-titlebar": "38px",
  "--lt-max-width": "1140px",
  "--l-bg": "var(--lt-bg)",
  "--l-bg-2": "var(--lt-bg-raise)",
  "--l-panel": "var(--lt-panel)",
  "--l-panel-2": "var(--lt-panel)",
  "--l-line": "var(--lt-line)",
  "--l-line-strong": "var(--lt-line-strong)",
  "--l-border": "var(--lt-border)",
  "--l-text": "var(--lt-text)",
  "--l-text-soft": "var(--lt-text-soft)",
  "--l-muted": "var(--lt-muted)",
  "--l-faint": "var(--lt-faint)",
  "--l-amber": "var(--lt-accent)",
  "--l-amber-bright": "var(--lt-accent-bright)",
  "--l-amber-deep": "var(--lt-accent-deep)",
  "--l-amber-glow": "var(--lt-accent-glow)",
  "--l-amber-wash": "var(--lt-accent-wash)",
  "--l-font-display": "var(--lt-font-display)",
  "--l-font-body": "var(--lt-font-ui)",
  "--l-font-mono": "var(--lt-font-mono)",
  "--l-maxw": "var(--lt-max-width)",
  "--l-radius": "var(--lt-radius-lg)",
  "--l-radius-sm": "var(--lt-radius)",
  "--l-grid-size": "var(--lt-grid)",
};

function parseCssRootVars(css: string): Record<string, string> {
  const vars: Record<string, string> = {};
  const re = /(--[a-z0-9-]+)\s*:\s*([^;]+);/gi;
  let m: RegExpExecArray | null;
  while ((m = re.exec(css))) {
    vars[m[1]] = m[2].trim();
  }
  return vars;
}

const SLATE_VARS = parseCssRootVars(slateCss);

function readPrefs(): DemoPrefs {
  try {
    const raw = localStorage.getItem(DEMO_PREFS_KEY);
    if (!raw) return { ...DEFAULT_PREFS };
    return { ...DEFAULT_PREFS, ...(JSON.parse(raw) as DemoPrefs) };
  } catch {
    return { ...DEFAULT_PREFS };
  }
}

function writePrefs(prefs: DemoPrefs): void {
  localStorage.setItem(DEMO_PREFS_KEY, JSON.stringify(prefs));
}

function builtins() {
  return [
    {
      id: "lattice-slate",
      name: "Lattice Slate",
      appearance: "dark",
      source: "builtin" as const,
      path: "builtin:lattice-slate.theme.yaml",
    },
    {
      id: "lattice-paper",
      name: "Lattice Paper",
      appearance: "light",
      source: "builtin" as const,
      path: "builtin:lattice-paper.theme.yaml",
    },
  ];
}

function resolveId(prefs: DemoPrefs, system: "dark" | "light"): string {
  if (prefs.mode === "auto") {
    return system === "light" ? prefs.pair.light : prefs.pair.dark;
  }
  return prefs.theme;
}

function buildResolved(
  id: string,
  prefs: DemoPrefs,
): ResolvedThemePayload {
  const isPaper = id === "lattice-paper";
  const vars = isPaper ? { ...PAPER_VARS } : { ...SLATE_VARS };
  return {
    id,
    name: isPaper ? "Lattice Paper" : "Lattice Slate",
    appearance: isPaper ? "light" : "dark",
    sourcePath: `builtin:${id}.theme.yaml`,
    vars,
    background: vars["--lt-bg"] ?? "#0a0d13",
    settings: {
      mode: prefs.mode,
      theme: prefs.theme,
      pair: { ...prefs.pair },
    },
    workspaceOverride: {},
    diagnostics: [],
  };
}

export function demoCatalog(
  system: "dark" | "light" = detectSystemAppearance(),
  _workspaceRoot?: string | null,
): ThemeCatalogPayload {
  const prefs = readPrefs();
  const id = resolveId(prefs, system);
  return {
    themes: builtins(),
    diagnostics: [],
    resolved: buildResolved(id, prefs),
  };
}

export function demoSetTheme(
  themeId: string,
  system: "dark" | "light",
): ThemeCatalogPayload {
  const prefs = { ...readPrefs(), mode: "fixed" as const, theme: themeId };
  writePrefs(prefs);
  return demoCatalog(system);
}

export function demoSetAppearanceMode(
  mode: "fixed" | "auto",
  system: "dark" | "light",
): ThemeCatalogPayload {
  const prefs = { ...readPrefs(), mode };
  writePrefs(prefs);
  return demoCatalog(system);
}
