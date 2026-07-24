// Theme-driven Vega-Lite config — resolves live `--lt-*` tokens to concrete colors.
// Vega cannot parse color-mix()/var() expressions, so every color is resolved
// through a DOM probe before it enters the config.
import type { Config } from "vega-lite";

import { parseFontStack, readToken } from "../canvas/colors";
import {
  FONT_DISPLAY as FALLBACK_FONT_DISPLAY,
  FONT_UI as FALLBACK_FONT_UI,
  LINE as FALLBACK_LINE,
  LINE_STRONG as FALLBACK_LINE_STRONG,
  LT,
  MUTED as FALLBACK_MUTED,
  TEXT as FALLBACK_TEXT,
  TEXT_SOFT as FALLBACK_TEXT_SOFT,
} from "../theme-tokens";

/** ANSI palette tokens, present on ~10 terminal-flavored themes only. */
const ANSI_TOKENS = [
  "--lt-term-blue",
  "--lt-term-cyan",
  "--lt-term-green",
  "--lt-term-magenta",
  "--lt-term-yellow",
  "--lt-term-red",
  "--lt-term-bright-blue",
  "--lt-term-bright-cyan",
  "--lt-term-bright-green",
  "--lt-term-bright-magenta",
  "--lt-term-bright-yellow",
  "--lt-term-bright-red",
];

/** Resolve any CSS color expression (var()/color-mix()) to concrete rgb() via a DOM probe. */
export function resolveCssColor(value: string): string | null {
  if (typeof document === "undefined" || !value) return null;
  const probe = document.createElement("span");
  probe.style.display = "none";
  probe.style.color = value;
  if (!probe.style.color) return null;
  document.body.appendChild(probe);
  const resolved = getComputedStyle(probe).color;
  probe.remove();
  return resolved || null;
}

/** Resolve a `--lt-*` token to concrete rgb(); missing tokens fall back statically. */
function themeColor(token: string, fallback: string): string {
  const raw = readToken(token, "");
  if (!raw) return fallback;
  return resolveCssColor(raw) ?? fallback;
}

/** Join a font stack into a CSS font-family string, quoting multi-word names. */
function toFontFamily(raw: string, fallback: readonly string[]): string {
  const families = parseFontStack(raw, [...fallback]);
  return families.map((f) => (/\s/.test(f) ? `"${f}"` : f)).join(", ");
}

function parseRgb(color: string): [number, number, number] | null {
  const m = /rgba?\(\s*([\d.]+)[,\s]+([\d.]+)[,\s]+([\d.]+)/.exec(color);
  return m ? [Number(m[1]), Number(m[2]), Number(m[3])] : null;
}

/** Drop entries within a just-noticeable RGB distance of an already-kept color. */
function dedupeColors(colors: string[], minDistance = 30): string[] {
  const kept: string[] = [];
  const keptRgb: [number, number, number][] = [];
  for (const color of colors) {
    const rgb = parseRgb(color);
    if (!rgb) {
      if (!kept.includes(color)) kept.push(color);
      continue;
    }
    const tooClose = keptRgb.some(
      (other) => Math.hypot(rgb[0] - other[0], rgb[1] - other[1], rgb[2] - other[2]) < minDistance,
    );
    if (!tooClose) {
      kept.push(color);
      keptRgb.push(rgb);
    }
  }
  return kept;
}

/** Categorical range: resolved ANSI tokens when the theme ships them, accent ramp otherwise. */
function buildCategoryRange(accent: string): string[] {
  const ansi = ANSI_TOKENS.map((token) => {
    const raw = readToken(token, "");
    return raw ? resolveCssColor(raw) : null;
  }).filter((color): color is string => color !== null);
  if (ansi.length >= 5) {
    return dedupeColors([accent, ...ansi]).slice(0, 10);
  }

  // No ANSI palette: rotate the accent family against slate/text so adjacent
  // entries stay distinguishable and legible on --lt-panel in light and dark.
  const ramp = [
    "var(--lt-accent)",
    "var(--lt-slate)",
    "var(--lt-accent-bright)",
    "color-mix(in oklch, var(--lt-accent) 55%, var(--lt-text) 45%)",
    "var(--lt-accent-deep)",
    "color-mix(in oklch, var(--lt-slate) 60%, var(--lt-text) 40%)",
    "color-mix(in oklch, var(--lt-accent) 40%, var(--lt-slate) 60%)",
    "color-mix(in oklch, var(--lt-accent-bright) 45%, var(--lt-text) 55%)",
    "color-mix(in oklch, var(--lt-accent-deep) 60%, var(--lt-slate) 40%)",
    "color-mix(in oklch, var(--lt-slate) 75%, var(--lt-bg) 25%)",
  ]
    .map((expr) => resolveCssColor(expr))
    .filter((color): color is string => color !== null);
  const palette = dedupeColors(ramp);
  return palette.length > 0
    ? palette
    : [LT.accent, LT.slate, LT.accentBright, LT.accentDeep, LT.textSoft, LT.muted];
}

/** Snapshot the live `--lt-*` theme into a Vega-Lite config (call per embed). */
export function buildVegaConfig(): Config {
  const text = themeColor("--lt-text", FALLBACK_TEXT);
  const textSoft = themeColor("--lt-text-soft", FALLBACK_TEXT_SOFT);
  const muted = themeColor("--lt-muted", FALLBACK_MUTED);
  const line = themeColor("--lt-line", FALLBACK_LINE);
  const lineStrong = themeColor("--lt-line-strong", FALLBACK_LINE_STRONG);
  const accent = themeColor("--lt-accent", LT.accent);
  const fontUi = toFontFamily(readToken("--lt-font-ui", ""), FALLBACK_FONT_UI);
  const fontDisplay = toFontFamily(
    readToken("--lt-font-display", readToken("--lt-font-ui", "")),
    FALLBACK_FONT_DISPLAY,
  );

  return {
    background: "transparent",
    view: { stroke: "transparent" },
    axis: {
      labelColor: muted,
      titleColor: textSoft,
      domainColor: lineStrong,
      tickColor: lineStrong,
      gridColor: line,
      labelFont: fontUi,
      titleFont: fontUi,
      labelFontSize: 11,
      titleFontSize: 12,
      titleFontWeight: 600,
    },
    legend: {
      labelColor: textSoft,
      titleColor: text,
      labelFont: fontUi,
      titleFont: fontUi,
    },
    title: {
      color: text,
      font: fontDisplay,
      fontWeight: 600,
      fontSize: 14,
      anchor: "start",
    },
    range: { category: buildCategoryRange(accent) },
    mark: { color: accent },
    bar: { color: accent },
    area: { color: accent },
    line: { color: accent },
    point: { color: accent },
    rect: { color: accent },
  };
}
