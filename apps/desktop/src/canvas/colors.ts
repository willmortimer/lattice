// Live canvas palette for Pixi — reads CSS `--lt-*` roles at paint time.
// Derived washes are recomputed as rgba() because Pixi cannot resolve color-mix().
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
  FONT_UI: string[];
  FONT_MONO: string[];
  FONT_DISPLAY: string[];
}

function readToken(name: string, fallback: string): string {
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

function parseFontStack(value: string, fallback: string[]): string[] {
  const parts = value
    .split(",")
    .map((part) => part.trim().replace(/^["']|["']$/g, ""))
    .filter(Boolean);
  return parts.length > 0 ? parts : fallback;
}

/** Snapshot the active shell theme into Pixi-safe color/font values. */
export function readCanvasPalette(): CanvasPalette {
  const slate = readToken("--lt-slate", LT.slate);
  const accent = readToken("--lt-accent", LT.accent);

  return {
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
