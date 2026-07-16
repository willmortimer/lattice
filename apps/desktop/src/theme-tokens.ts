/* GENERATED from themes/lattice-slate.theme.yaml — do not edit by hand.
 * Recompile: node scripts/compile-theme.mjs
 * Theme: Lattice Slate (lattice-slate)
 *
 * Pixi/canvas cannot read CSS variables; these mirror --lt-* roles with
 * precomputed rgba washes that match the color-mix alphas in theme-tokens.css.
 */

export const LT = {
  bg: "#0a0d13",
  bgRaise: "#0e1320",
  panel: "#131a29",
  slate: "#8ca2c4",
  text: "#e7ecf5",
  textSoft: "#b9c2d4",
  muted: "#8791a6",
  faint: "#5f6a80",
  accent: "#f5a623",
  accentBright: "#ffce8a",
  accentDeep: "#d98615",
  danger: "#ff9d8a",
  shadow: "#000000",
  onAccent: "#201403",
} as const;

export const PANEL = LT.panel;
export const BG_RAISE = LT.bgRaise;
export const BORDER = "rgba(140, 162, 196, 0.18)";
export const LINE = "rgba(140, 162, 196, 0.12)";
export const LINE_STRONG = "rgba(140, 162, 196, 0.22)";

export const AMBER = LT.accent;
export const AMBER_BRIGHT = LT.accentBright;
export const AMBER_DEEP = LT.accentDeep;
export const AMBER_WASH = "rgba(245, 166, 35, 0.1)";

export const TEXT = LT.text;
export const TEXT_SOFT = LT.textSoft;
export const MUTED = LT.muted;
export const FAINT = LT.faint;

export const FONT_UI = ["Space Grotesk Variable", "Space Grotesk", "system-ui", "sans-serif"];
export const FONT_MONO = ["JetBrains Mono Variable", "JetBrains Mono", "ui-monospace", "SF Mono", "Menlo", "monospace"];
export const FONT_DISPLAY = ["Fraunces Variable", "Fraunces", "Georgia", "serif"];
