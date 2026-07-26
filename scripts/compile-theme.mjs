#!/usr/bin/env node
/**
 * Compile a Lattice theme YAML into CSS custom properties (`--lt-*`).
 *
 *   pnpm compile-theme
 *   nix run .#compile-theme
 *
 * Default theme: themes/lattice-slate.theme.yaml
 * Writes:
 *   apps/desktop/src/theme-tokens.css
 *   apps/desktop/src/theme-tokens.ts  (Pixi/canvas mirror)
 *   site/src/styles/theme-tokens.css
 *
 * Parser is intentionally tiny: themes are a constrained YAML subset
 * (scalars + one-level maps). No runtime deps — same spirit as generate-mark.
 */

import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(__dirname, "..");

const DEFAULT_THEME = join(ROOT, "themes", "lattice-slate.theme.yaml");
const FONT_PACKS_DIR = join(ROOT, "themes", "font-packs");
const OUT_DESKTOP = join(ROOT, "apps", "desktop", "src", "theme-tokens.css");
const OUT_SITE = join(ROOT, "site", "src", "styles", "theme-tokens.css");
const OUT_DESKTOP_TS = join(ROOT, "apps", "desktop", "src", "theme-tokens.ts");
const DEFAULT_FONT_PACK_ID = "lattice";
const FONT_PACK_SUFFIX = ".font-pack.yaml";

// ---------------------------------------------------------------------------
// Minimal YAML (maps + scalars only)
// ---------------------------------------------------------------------------

function parseScalar(raw) {
  const s = raw.trim();
  if (
    (s.startsWith('"') && s.endsWith('"')) ||
    (s.startsWith("'") && s.endsWith("'"))
  ) {
    return s.slice(1, -1);
  }
  if (s === "true") return true;
  if (s === "false") return false;
  if (/^-?\d+(\.\d+)?$/.test(s)) return Number(s);
  return s;
}

/**
 * Parse a constrained theme YAML document into a nested object.
 * Supports comments, top-level scalars, and one-level nested maps.
 */
function parseThemeYaml(source) {
  const root = {};
  /** @type {Record<string, unknown> | null} */
  let currentMap = null;
  /** @type {string | null} */
  let currentKey = null;

  for (const line of source.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;

    const indent = line.match(/^ */)?.[0].length ?? 0;
    if (indent > 0) {
      if (!currentMap || currentKey === null) {
        throw new Error(`Unexpected indented line: ${trimmed}`);
      }
      const m = trimmed.match(/^([A-Za-z0-9_]+):\s*(.*)$/);
      if (!m) throw new Error(`Bad map entry: ${trimmed}`);
      const [, k, rest] = m;
      if (rest === "") {
        throw new Error(`Nested maps deeper than one level are not supported: ${trimmed}`);
      }
      currentMap[k] = parseScalar(rest);
      continue;
    }

    const m = trimmed.match(/^([A-Za-z0-9_]+):\s*(.*)$/);
    if (!m) throw new Error(`Bad top-level line: ${trimmed}`);
    const [, k, rest] = m;
    if (rest === "" || rest === "{}") {
      currentKey = k;
      currentMap = {};
      root[k] = currentMap;
      continue;
    }
    // Inline map: { a: b, c: d } — not used by themes today; reject clearly.
    if (rest.startsWith("{")) {
      throw new Error(`Inline maps are not supported (use nested keys): ${k}`);
    }
    currentKey = null;
    currentMap = null;
    root[k] = parseScalar(rest);
  }

  return root;
}

// ---------------------------------------------------------------------------
// Resolve + validate
// ---------------------------------------------------------------------------

function resolveRef(value, palette) {
  if (typeof value !== "string") return String(value);
  if (value.startsWith("$")) {
    const key = value.slice(1);
    if (!(key in palette)) {
      throw new Error(`Unknown palette ref $${key}`);
    }
    return String(palette[key]);
  }
  return value;
}

function requireKeys(obj, keys, label) {
  for (const k of keys) {
    if (obj[k] === undefined || obj[k] === null || obj[k] === "") {
      throw new Error(`${label} missing required key: ${k}`);
    }
  }
}

function validateTheme(theme) {
  requireKeys(theme, ["name", "id", "appearance", "palette", "roles", "font_pack", "shape"], "theme");
  if (theme.appearance !== "dark" && theme.appearance !== "light") {
    throw new Error(`appearance must be dark|light, got ${theme.appearance}`);
  }
  if (typeof theme.palette !== "object" || Array.isArray(theme.palette)) {
    throw new Error("palette must be a map");
  }
  if (typeof theme.roles !== "object" || Array.isArray(theme.roles)) {
    throw new Error("roles must be a map");
  }
  if (typeof theme.font_pack !== "string" || !theme.font_pack.trim()) {
    throw new Error("font_pack must be a non-empty string");
  }
  requireKeys(
    theme.shape,
    ["radius", "radius_sm", "radius_lg", "grid", "titlebar", "max_width"],
    "shape",
  );

  const roleKeys = [
    "bg",
    "bg_raise",
    "panel",
    "slate",
    "text",
    "text_soft",
    "muted",
    "faint",
    "accent",
    "accent_bright",
    "accent_deep",
    "danger",
    "shadow",
  ];
  requireKeys(theme.roles, roleKeys, "roles");

  if (theme.terminal !== undefined) {
    if (typeof theme.terminal !== "object" || Array.isArray(theme.terminal)) {
      throw new Error("terminal must be a map");
    }
    requireKeys(theme.terminal, TERMINAL_ANSI_KEYS, "terminal");
  }
}

function validateFontPack(pack) {
  requireKeys(pack, ["name", "id", "fonts"], "font pack");
  requireKeys(pack.fonts, ["display", "ui", "mono"], "fonts");
}

/** Load a font pack YAML by id from themes/font-packs/. */
function loadFontPack(packId) {
  const id = String(packId || DEFAULT_FONT_PACK_ID).trim() || DEFAULT_FONT_PACK_ID;
  const packPath = join(FONT_PACKS_DIR, `${id}${FONT_PACK_SUFFIX}`);
  let source;
  try {
    source = readFileSync(packPath, "utf8");
  } catch {
    throw new Error(`font pack not found: ${id} (${packPath})`);
  }
  const pack = parseThemeYaml(source);
  validateFontPack(pack);
  if (pack.id !== id) {
    throw new Error(`font pack id ${pack.id} does not match filename id ${id}`);
  }
  return pack;
}

/** Resolve theme.font_pack into theme.fonts for emitCss / emitTs. */
function attachResolvedFonts(theme) {
  const pack = loadFontPack(theme.font_pack);
  theme.fonts = pack.fonts;
  theme.resolved_font_pack = pack.id;
  return theme;
}

/** ANSI slots a `terminal:` block must provide when present. */
const TERMINAL_ANSI_KEYS = [
  "black",
  "red",
  "green",
  "yellow",
  "blue",
  "magenta",
  "cyan",
  "white",
  "bright_black",
  "bright_red",
  "bright_green",
  "bright_yellow",
  "bright_blue",
  "bright_magenta",
  "bright_cyan",
  "bright_white",
];

// ---------------------------------------------------------------------------
// Emit CSS
// ---------------------------------------------------------------------------

function emitCss(theme, sourcePath) {
  const palette = theme.palette;
  /** @type {Record<string, string>} */
  const roles = {};
  for (const [k, v] of Object.entries(theme.roles)) {
    roles[k] = resolveRef(v, palette);
  }

  const onAccent =
    roles.on_accent ??
    (typeof palette.on_accent === "string" ? palette.on_accent : "#201403");

  const rel = sourcePath.startsWith(ROOT)
    ? sourcePath.slice(ROOT.length + 1)
    : sourcePath;

  const terminalLines = theme.terminal
    ? [
        ``,
        `  /* Terminal (ANSI) — from the theme's terminal: block */`,
        ...Object.entries(theme.terminal).map(
          ([k, v]) =>
            `  --lt-term-${k.replace(/_/g, "-")}: ${resolveRef(v, palette)};`,
        ),
      ]
    : [];

  const lines = [
    `/* GENERATED from ${rel} — do not edit by hand.`,
    `   Recompile: pnpm compile-theme (or nix run .#compile-theme)`,
    `   Theme: ${theme.name} (${theme.id})`,
    `   Font pack: ${theme.resolved_font_pack ?? theme.font_pack} */`,
    ``,
    `:root {`,
    `  color-scheme: ${theme.appearance};`,
    ``,
    `  /* Roles */`,
    `  --lt-bg: ${roles.bg};`,
    `  --lt-bg-raise: ${roles.bg_raise};`,
    `  --lt-panel: ${roles.panel};`,
    `  --lt-slate: ${roles.slate};`,
    `  --lt-text: ${roles.text};`,
    `  --lt-text-soft: ${roles.text_soft};`,
    `  --lt-muted: ${roles.muted};`,
    `  --lt-faint: ${roles.faint};`,
    `  --lt-accent: ${roles.accent};`,
    `  --lt-accent-bright: ${roles.accent_bright};`,
    `  --lt-accent-deep: ${roles.accent_deep};`,
    `  --lt-danger: ${roles.danger};`,
    `  --lt-shadow: ${roles.shadow};`,
    `  --lt-on-accent: ${onAccent};`,
    `  --lt-surface: var(--lt-panel);`,
    ``,
    `  /* Derived — color-mix keeps washes/glows in lockstep with roles */`,
    `  --lt-hover: color-mix(in oklch, var(--lt-slate) 7%, transparent);`,
    `  --lt-line: color-mix(in oklch, var(--lt-slate) 12%, transparent);`,
    `  --lt-line-strong: color-mix(in oklch, var(--lt-slate) 22%, transparent);`,
    `  --lt-border: color-mix(in oklch, var(--lt-slate) 18%, transparent);`,
    `  --lt-accent-wash: color-mix(in oklch, var(--lt-accent) 10%, transparent);`,
    `  --lt-accent-glow: color-mix(in oklch, var(--lt-accent) 55%, transparent);`,
    `  --lt-accent-glow-soft: color-mix(in oklch, var(--lt-accent) 35%, transparent);`,
    `  --lt-accent-glow-mid: color-mix(in oklch, var(--lt-accent) 22%, transparent);`,
    `  --lt-accent-glow-strong: color-mix(in oklch, var(--lt-accent) 45%, transparent);`,
    `  --lt-accent-underline: color-mix(in oklch, var(--lt-accent) 35%, transparent);`,
    `  --lt-node-dot: color-mix(in oklch, var(--lt-slate) 26%, transparent);`,
    `  --lt-node-dot-soft: color-mix(in oklch, var(--lt-slate) 20%, transparent);`,
    `  --lt-scrim: color-mix(in oklch, var(--lt-bg) 60%, transparent);`,
    `  --lt-scrim-deep: color-mix(in oklch, var(--lt-bg) 72%, #06080c);`,
    `  --lt-shadow-md: color-mix(in oklch, var(--lt-shadow) 35%, transparent);`,
    `  --lt-shadow-lg: color-mix(in oklch, var(--lt-shadow) 45%, transparent);`,
    ...terminalLines,
    ``,
    `  /* Fonts */`,
    `  --lt-font-display: ${theme.fonts.display};`,
    `  --lt-font-ui: ${theme.fonts.ui};`,
    `  --lt-font-mono: ${theme.fonts.mono};`,
    ``,
    `  /* Shape */`,
    `  --lt-radius: ${theme.shape.radius};`,
    `  --lt-radius-sm: ${theme.shape.radius_sm};`,
    `  --lt-radius-lg: ${theme.shape.radius_lg};`,
    `  --lt-grid: ${theme.shape.grid};`,
    `  --lt-titlebar: ${theme.shape.titlebar};`,
    `  --lt-max-width: ${theme.shape.max_width};`,
    ``,
    `  /* Token tier — theme-independent scale + surfaces derived from live roles.`,
    `     Static values never vary by theme; derived ones use var() so they track`,
    `     whichever theme is active. Safe to regenerate per-theme (idempotent). */`,
    `  --lt-space-1: 4px;`,
    `  --lt-space-2: 8px;`,
    `  --lt-space-3: 12px;`,
    `  --lt-space-4: 16px;`,
    `  --lt-space-5: 24px;`,
    `  --lt-space-6: 32px;`,
    `  --lt-font-size-xs: 11px;`,
    `  --lt-font-size-sm: 12px;`,
    `  --lt-font-size-md: 13.5px;`,
    `  --lt-font-size-lg: 16px;`,
    `  --lt-font-weight-medium: 500;`,
    `  --lt-font-weight-semibold: 600;`,
    `  --lt-radius-xs: 4px;`,
    `  --lt-radius-md: var(--lt-radius);`,
    `  --lt-surface-raised: var(--lt-bg-raise);`,
    `  --lt-surface-sunken: color-mix(in oklch, var(--lt-bg) 94%, black);`,
    `  --lt-focus-ring: var(--lt-accent-bright);`,
    `  --lt-warning: var(--lt-accent-deep);`,
    `  --lt-success: var(--lt-term-green, #4e9a6c);`,
    ``,
    `  /* Site aliases (--l-*) — same roles, legacy names used by marketing CSS */`,
    `  --l-bg: var(--lt-bg);`,
    `  --l-bg-2: var(--lt-bg-raise);`,
    `  --l-panel: var(--lt-panel);`,
    `  --l-panel-2: var(--lt-panel);`,
    `  --l-line: var(--lt-line);`,
    `  --l-line-strong: var(--lt-line-strong);`,
    `  --l-border: var(--lt-border);`,
    `  --l-text: var(--lt-text);`,
    `  --l-text-soft: var(--lt-text-soft);`,
    `  --l-muted: var(--lt-muted);`,
    `  --l-faint: var(--lt-faint);`,
    `  --l-amber: var(--lt-accent);`,
    `  --l-amber-bright: var(--lt-accent-bright);`,
    `  --l-amber-deep: var(--lt-accent-deep);`,
    `  --l-amber-glow: var(--lt-accent-glow);`,
    `  --l-amber-wash: var(--lt-accent-wash);`,
    `  --l-font-display: var(--lt-font-display);`,
    `  --l-font-body: var(--lt-font-ui);`,
    `  --l-font-mono: var(--lt-font-mono);`,
    `  --l-maxw: var(--lt-max-width);`,
    `  --l-radius: var(--lt-radius-lg);`,
    `  --l-radius-sm: var(--lt-radius);`,
    `  --l-grid-size: var(--lt-grid);`,
    `}`,
    ``,
  ];

  return lines.join("\n");
}

/** Parse #RRGGBB into rgba() for Pixi / canvas (no CSS color-mix there). */
function hexToRgba(hex, alpha) {
  const h = String(hex).replace("#", "");
  if (h.length !== 6) {
    throw new Error(`hexToRgba expects #RRGGBB, got ${hex}`);
  }
  const r = Number.parseInt(h.slice(0, 2), 16);
  const g = Number.parseInt(h.slice(2, 4), 16);
  const b = Number.parseInt(h.slice(4, 6), 16);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

/** Strip CSS stack quotes into a JS string-array literal source. */
function fontStackToJsArray(stack) {
  const parts = String(stack)
    .split(",")
    .map((p) => p.trim().replace(/^["']|["']$/g, ""))
    .filter(Boolean);
  return `[${parts.map((p) => JSON.stringify(p)).join(", ")}]`;
}

function emitTs(theme, sourcePath) {
  const palette = theme.palette;
  /** @type {Record<string, string>} */
  const roles = {};
  for (const [k, v] of Object.entries(theme.roles)) {
    roles[k] = resolveRef(v, palette);
  }
  const onAccent =
    roles.on_accent ??
    (typeof palette.on_accent === "string" ? palette.on_accent : "#201403");

  const rel = sourcePath.startsWith(ROOT)
    ? sourcePath.slice(ROOT.length + 1)
    : sourcePath;

  return `/* GENERATED from ${rel} — do not edit by hand.
 * Recompile: node scripts/compile-theme.mjs
 * Theme: ${theme.name} (${theme.id})
 * Font pack: ${theme.resolved_font_pack ?? theme.font_pack}
 *
 * Pixi/canvas cannot read CSS variables; these mirror --lt-* roles with
 * precomputed rgba washes that match the color-mix alphas in theme-tokens.css.
 */

export const LT = {
  bg: ${JSON.stringify(roles.bg)},
  bgRaise: ${JSON.stringify(roles.bg_raise)},
  panel: ${JSON.stringify(roles.panel)},
  slate: ${JSON.stringify(roles.slate)},
  text: ${JSON.stringify(roles.text)},
  textSoft: ${JSON.stringify(roles.text_soft)},
  muted: ${JSON.stringify(roles.muted)},
  faint: ${JSON.stringify(roles.faint)},
  accent: ${JSON.stringify(roles.accent)},
  accentBright: ${JSON.stringify(roles.accent_bright)},
  accentDeep: ${JSON.stringify(roles.accent_deep)},
  danger: ${JSON.stringify(roles.danger)},
  shadow: ${JSON.stringify(roles.shadow)},
  onAccent: ${JSON.stringify(onAccent)},
} as const;

export const PANEL = LT.panel;
export const BG_RAISE = LT.bgRaise;
export const BORDER = ${JSON.stringify(hexToRgba(roles.slate, 0.18))};
export const LINE = ${JSON.stringify(hexToRgba(roles.slate, 0.12))};
export const LINE_STRONG = ${JSON.stringify(hexToRgba(roles.slate, 0.22))};

export const AMBER = LT.accent;
export const AMBER_BRIGHT = LT.accentBright;
export const AMBER_DEEP = LT.accentDeep;
export const AMBER_WASH = ${JSON.stringify(hexToRgba(roles.accent, 0.1))};

export const TEXT = LT.text;
export const TEXT_SOFT = LT.textSoft;
export const MUTED = LT.muted;
export const FAINT = LT.faint;

export const FONT_UI = ${fontStackToJsArray(theme.fonts.ui)};
export const FONT_MONO = ${fontStackToJsArray(theme.fonts.mono)};
export const FONT_DISPLAY = ${fontStackToJsArray(theme.fonts.display)};
`;
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

function main() {
  const themePath = resolve(process.argv[2] ?? DEFAULT_THEME);
  const source = readFileSync(themePath, "utf8");
  const theme = parseThemeYaml(source);
  validateTheme(theme);
  attachResolvedFonts(theme);
  const css = emitCss(theme, themePath);
  const ts = emitTs(theme, themePath);

  for (const out of [OUT_DESKTOP, OUT_SITE]) {
    mkdirSync(dirname(out), { recursive: true });
    writeFileSync(out, css, "utf8");
    console.log(`wrote ${out.slice(ROOT.length + 1)}`);
  }

  mkdirSync(dirname(OUT_DESKTOP_TS), { recursive: true });
  writeFileSync(OUT_DESKTOP_TS, ts, "utf8");
  console.log(`wrote ${OUT_DESKTOP_TS.slice(ROOT.length + 1)}`);
}

main();
