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

function variantVars(overrides: Record<string, string>): Record<string, string> {
  return { ...SLATE_VARS, ...overrides };
}

const DEMO_THEMES: Record<
  string,
  { name: string; appearance: "dark" | "light"; vars: Record<string, string> }
> = {
  "lattice-slate": { name: "Lattice Slate", appearance: "dark", vars: SLATE_VARS },
  "lattice-paper": { name: "Lattice Paper", appearance: "light", vars: PAPER_VARS },
  "lattice-carbon": {
    name: "Lattice Carbon",
    appearance: "dark",
    vars: variantVars({
      "--lt-bg": "#090a0c",
      "--lt-bg-raise": "#101215",
      "--lt-panel": "#171a1e",
      "--lt-slate": "#98a0aa",
      "--lt-text": "#f0f2f4",
      "--lt-text-soft": "#c5c9ce",
      "--lt-muted": "#8e959e",
      "--lt-faint": "#626973",
      "--lt-accent": "#ff7a45",
      "--lt-accent-bright": "#ffb08f",
      "--lt-accent-deep": "#d95222",
      "--lt-danger": "#ff806f",
      "--lt-on-accent": "#240b03",
      "--lt-radius": "7px",
      "--lt-radius-sm": "5px",
      "--lt-radius-lg": "12px",
      "--lt-grid": "32px",
    }),
  },
  "lattice-fjord": {
    name: "Lattice Fjord",
    appearance: "dark",
    vars: variantVars({
      "--lt-bg": "#071217",
      "--lt-bg-raise": "#0b1a21",
      "--lt-panel": "#10252d",
      "--lt-slate": "#88a8b3",
      "--lt-text": "#e8f3f4",
      "--lt-text-soft": "#b8cdd1",
      "--lt-muted": "#7f9ba3",
      "--lt-faint": "#536f78",
      "--lt-accent": "#42d6bd",
      "--lt-accent-bright": "#9aefdf",
      "--lt-accent-deep": "#159b89",
      "--lt-danger": "#ff8e82",
      "--lt-on-accent": "#04201c",
      "--lt-radius": "10px",
      "--lt-radius-sm": "7px",
      "--lt-radius-lg": "15px",
      "--lt-grid": "36px",
    }),
  },
  "lattice-ultraviolet": {
    name: "Lattice Ultraviolet",
    appearance: "dark",
    vars: variantVars({
      "--lt-bg": "#0d0916",
      "--lt-bg-raise": "#151023",
      "--lt-panel": "#1d1730",
      "--lt-slate": "#a199bd",
      "--lt-text": "#f1edfa",
      "--lt-text-soft": "#cbc3df",
      "--lt-muted": "#958aa9",
      "--lt-faint": "#695f7c",
      "--lt-accent": "#b08cff",
      "--lt-accent-bright": "#d8c5ff",
      "--lt-accent-deep": "#7651d5",
      "--lt-danger": "#ff8caa",
      "--lt-on-accent": "#180c2b",
      "--lt-radius": "11px",
      "--lt-radius-sm": "7px",
      "--lt-radius-lg": "17px",
    }),
  },
  "lattice-blueprint": {
    name: "Lattice Blueprint",
    appearance: "dark",
    vars: variantVars({
      "--lt-bg": "#07182b",
      "--lt-bg-raise": "#0b2139",
      "--lt-panel": "#102b49",
      "--lt-slate": "#84a7ca",
      "--lt-text": "#edf5ff",
      "--lt-text-soft": "#bfd3e8",
      "--lt-muted": "#88a6c2",
      "--lt-faint": "#587797",
      "--lt-accent": "#ffc857",
      "--lt-accent-bright": "#ffe3a0",
      "--lt-accent-deep": "#d99a20",
      "--lt-danger": "#ff9b8b",
      "--lt-on-accent": "#251801",
      "--lt-radius": "5px",
      "--lt-radius-sm": "3px",
      "--lt-radius-lg": "9px",
      "--lt-grid": "28px",
    }),
  },
  "lattice-vellum": {
    name: "Lattice Vellum",
    appearance: "light",
    vars: variantVars({
      "--lt-bg": "#f3ecdc",
      "--lt-bg-raise": "#ebe1ce",
      "--lt-panel": "#fbf7ed",
      "--lt-slate": "#766f66",
      "--lt-text": "#2b241d",
      "--lt-text-soft": "#554b40",
      "--lt-muted": "#786d61",
      "--lt-faint": "#9a8d7d",
      "--lt-accent": "#9f3f35",
      "--lt-accent-bright": "#7d2d27",
      "--lt-accent-deep": "#6b211d",
      "--lt-danger": "#a62f2a",
      "--lt-on-accent": "#fff8ee",
      "--lt-radius": "8px",
      "--lt-radius-sm": "5px",
      "--lt-radius-lg": "12px",
    }),
  },
};

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
  return Object.entries(DEMO_THEMES).map(([id, theme]) => ({
    id,
    name: theme.name,
    appearance: theme.appearance,
    source: "builtin" as const,
    path: `builtin:${id}.theme.yaml`,
  }));
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
  const theme = DEMO_THEMES[id] ?? DEMO_THEMES["lattice-slate"];
  const vars = { ...theme.vars };
  return {
    id,
    name: theme.name,
    appearance: theme.appearance,
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
