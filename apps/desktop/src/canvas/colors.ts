// Live canvas palette for Pixi — reads CSS `--lt-*` roles at paint time.
// Derived washes are recomputed as rgba() because Pixi cannot resolve color-mix().
import type { ResourceKind } from "../types";
import {
  AMBER_BRIGHT as FALLBACK_AMBER_BRIGHT,
  AMBER_DEEP as FALLBACK_AMBER_DEEP,
  AMBER_WASH as FALLBACK_AMBER_WASH,
  BG_RAISE as FALLBACK_BG_RAISE,
  BORDER as FALLBACK_BORDER,
  FAINT as FALLBACK_FAINT,
  FONT_DISPLAY as FALLBACK_FONT_DISPLAY,
  FONT_MONO as FALLBACK_FONT_MONO,
  FONT_UI as FALLBACK_FONT_UI,
  LINE as FALLBACK_LINE,
  LINE_STRONG as FALLBACK_LINE_STRONG,
  LT,
  MUTED as FALLBACK_MUTED,
  PANEL as FALLBACK_PANEL,
  TEXT as FALLBACK_TEXT,
  TEXT_SOFT as FALLBACK_TEXT_SOFT,
} from "../theme-tokens";

export interface CanvasPalette {
  BG: string;
  PANEL: string;
  BG_RAISE: string;
  BORDER: string;
  LINE: string;
  LINE_STRONG: string;
  AMBER: string;
  AMBER_BRIGHT: string;
  AMBER_DEEP: string;
  AMBER_WASH: string;
  TEXT: string;
  TEXT_SOFT: string;
  MUTED: string;
  FAINT: string;
  /** Painted card elevation (offset roundRects — Pixi has no drop-shadow filter here). */
  SHADOW: string;
  SHADOW_SOFT: string;
  /** Slate wash layered over a card fill on pointer hover. */
  HOVER: string;
  /** Very faint slate wash for group interiors. */
  GROUP_WASH: string;
  /** Low-alpha accent ring drawn outside a selected card. */
  ACCENT_GLOW: string;
  /** Dot-grid color for the camera-tracked background layer. */
  GRID_DOT: string;
  /** Base dot-grid spacing in world px (mirrors `--lt-grid`). */
  GRID_SIZE: number;
  /** Quiet per-kind accent hue for glyph chips + captions (hex). */
  KIND: Record<ResourceKind, string>;
  /** JSON Canvas preset colors "1".."6" → theme-aware hex. */
  PRESETS: Record<string, string>;
  FONT_UI: string[];
  FONT_MONO: string[];
  FONT_DISPLAY: string[];
}

/** Read a raw `--lt-*` custom property off `:root` (may be a color-mix() expression). */
export function readToken(name: string, fallback: string): string {
  if (typeof document === "undefined") return fallback;
  const value = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return value || fallback;
}

/** Parse `#RRGGBB` into `rgba()` for Pixi strokes/fills. */
export function hexToRgba(hex: string, alpha: number): string | null {
  const m = /^#([0-9a-fA-F]{2})([0-9a-fA-F]{2})([0-9a-fA-F]{2})$/.exec(hex.trim());
  if (!m) return null;
  const [r, g, b] = m.slice(1).map((h) => parseInt(h, 16));
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

function wash(hex: string, alpha: number, fallback: string): string {
  return hexToRgba(hex, alpha) ?? fallback;
}

/** Split a CSS font-family stack into unquoted family names. */
export function parseFontStack(value: string, fallback: string[]): string[] {
  const parts = value
    .split(",")
    .map((part) => part.trim().replace(/^["']|["']$/g, ""))
    .filter(Boolean);
  return parts.length > 0 ? parts : fallback;
}

/** Read a `--lt-*` token but only accept a Pixi-parseable #rrggbb value. */
function readHexToken(name: string, fallback: string): string {
  const value = readToken(name, "");
  return /^#[0-9a-fA-F]{6}$/.test(value) ? value : fallback;
}

/** Relative luminance of a #rrggbb color, or null when unparseable. */
function hexLuminance(hex: string): number | null {
  const m = /^#([0-9a-fA-F]{2})([0-9a-fA-F]{2})([0-9a-fA-F]{2})$/.exec(hex.trim());
  if (!m) return null;
  const [r, g, b] = m.slice(1).map((h) => parseInt(h, 16) / 255);
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

// Mid-tone fallback hues that hold up on both dark and light panels; a theme's
// own `--lt-term-*` palette wins when present so chips stay in-family.
const FALLBACK_HUES = {
  red: "#d9776b",
  orange: "#d98d4a",
  yellow: "#c9a94e",
  green: "#5fa671",
  cyan: "#55a8b5",
  blue: "#6f9edb",
  magenta: "#a98ad0",
} as const;

/** Snapshot the active shell theme into Pixi-safe color/font values. */
export function readCanvasPalette(): CanvasPalette {
  const bg = readToken("--lt-bg", LT.bg);
  const slate = readToken("--lt-slate", LT.slate);
  const accent = readToken("--lt-accent", LT.accent);
  const shadow = readHexToken("--lt-shadow", LT.shadow);
  // Light themes need far quieter shadows/washes than dark ones.
  const isLight = (hexLuminance(bg) ?? 0) > 0.5;

  const red = readHexToken("--lt-term-red", FALLBACK_HUES.red);
  const orange = readHexToken("--lt-term-orange", FALLBACK_HUES.orange);
  const yellow = readHexToken("--lt-term-yellow", FALLBACK_HUES.yellow);
  const green = readHexToken("--lt-term-green", FALLBACK_HUES.green);
  const cyan = readHexToken("--lt-term-cyan", FALLBACK_HUES.cyan);
  const blue = readHexToken("--lt-term-blue", FALLBACK_HUES.blue);
  const magenta = readHexToken("--lt-term-magenta", FALLBACK_HUES.magenta);

  return {
    BG: bg,
    PANEL: readToken("--lt-panel", FALLBACK_PANEL),
    BG_RAISE: readToken("--lt-bg-raise", FALLBACK_BG_RAISE),
    BORDER: wash(slate, 0.18, FALLBACK_BORDER),
    LINE: wash(slate, 0.12, FALLBACK_LINE),
    LINE_STRONG: wash(slate, 0.22, FALLBACK_LINE_STRONG),
    AMBER: accent,
    AMBER_BRIGHT: readToken("--lt-accent-bright", FALLBACK_AMBER_BRIGHT),
    AMBER_DEEP: readToken("--lt-accent-deep", FALLBACK_AMBER_DEEP),
    AMBER_WASH: wash(accent, 0.1, FALLBACK_AMBER_WASH),
    TEXT: readToken("--lt-text", FALLBACK_TEXT),
    TEXT_SOFT: readToken("--lt-text-soft", FALLBACK_TEXT_SOFT),
    MUTED: readToken("--lt-muted", FALLBACK_MUTED),
    FAINT: readToken("--lt-faint", FALLBACK_FAINT),
    SHADOW: wash(shadow, isLight ? 0.12 : 0.32, "rgba(0, 0, 0, 0.32)"),
    SHADOW_SOFT: wash(shadow, isLight ? 0.06 : 0.16, "rgba(0, 0, 0, 0.16)"),
    HOVER: wash(slate, 0.08, "rgba(140, 162, 196, 0.08)"),
    GROUP_WASH: wash(slate, 0.05, "rgba(140, 162, 196, 0.05)"),
    ACCENT_GLOW: wash(accent, isLight ? 0.3 : 0.24, "rgba(245, 166, 35, 0.24)"),
    GRID_DOT: wash(slate, 0.22, "rgba(140, 162, 196, 0.22)"),
    GRID_SIZE: parseFloat(readToken("--lt-grid", "34")) || 34,
    KIND: {
      page: blue,
      canvas: accent,
      "data-app": cyan,
      dataset: green,
      notebook: yellow,
      ink: magenta,
      artifact: orange,
      app: blue,
      workflow: cyan,
      task: green,
      derived: magenta,
      folder: slate,
      file: slate,
    },
    PRESETS: {
      "1": red,
      "2": orange,
      "3": yellow,
      "4": green,
      "5": cyan,
      "6": magenta,
    },
    FONT_UI: parseFontStack(readToken("--lt-font-ui", ""), [...FALLBACK_FONT_UI]),
    FONT_MONO: parseFontStack(readToken("--lt-font-mono", ""), [...FALLBACK_FONT_MONO]),
    FONT_DISPLAY: parseFontStack(readToken("--lt-font-display", ""), [...FALLBACK_FONT_DISPLAY]),
  };
}

/** Observe live theme swaps (`applyResolvedTheme` mutates `:root` style / data-theme). */
export function observeThemeChange(onChange: () => void): () => void {
  if (typeof document === "undefined" || typeof MutationObserver === "undefined") {
    return () => {};
  }
  let frame = 0;
  const observer = new MutationObserver(() => {
    cancelAnimationFrame(frame);
    frame = requestAnimationFrame(onChange);
  });
  observer.observe(document.documentElement, {
    attributes: true,
    attributeFilter: ["style", "data-theme", "class"],
  });
  return () => {
    cancelAnimationFrame(frame);
    observer.disconnect();
  };
}
