import type { ITheme } from "@xterm/xterm";

function readToken(name: string, fallback: string): string {
  if (typeof document === "undefined") return fallback;
  const value = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return value || fallback;
}

/**
 * Read an ANSI slot: prefer the theme's own `--lt-term-*` palette (set when a
 * theme declares a `terminal:` block, e.g. the adopted Catppuccin/Nord/GitHub
 * standards), falling back to the role-derived approximation.
 */
function ansi(slot: string, roleToken: string, fallback: string): string {
  const exact = readToken(`--lt-term-${slot}`, "");
  return exact || readToken(roleToken, fallback);
}

/** Map Lattice `--lt-*` tokens to an xterm.js theme. */
export function latticeTerminalTheme(): ITheme {
  return {
    background: readToken("--lt-bg", "#0a0d13"),
    foreground: readToken("--lt-text", "#e7ecf5"),
    cursor: ansi("cursor", "--lt-accent", "#f5a623"),
    cursorAccent: ansi("cursor-text", "--lt-on-accent", "#201403"),
    selectionBackground: ansi("selection", "--lt-accent-wash", "rgba(245, 166, 35, 0.1)"),
    black: ansi("black", "--lt-bg-raise", "#0e1320"),
    brightBlack: ansi("bright-black", "--lt-faint", "#5f6a80"),
    red: ansi("red", "--lt-danger", "#ff9d8a"),
    green: ansi("green", "--lt-accent-bright", "#ffce8a"),
    yellow: ansi("yellow", "--lt-accent", "#f5a623"),
    blue: ansi("blue", "--lt-slate", "#8ca2c4"),
    magenta: ansi("magenta", "--lt-text-soft", "#b9c2d4"),
    cyan: ansi("cyan", "--lt-muted", "#8791a6"),
    white: ansi("white", "--lt-text", "#e7ecf5"),
    brightRed: ansi("bright-red", "--lt-danger", "#ff9d8a"),
    brightGreen: ansi("bright-green", "--lt-accent-bright", "#ffce8a"),
    brightYellow: ansi("bright-yellow", "--lt-accent", "#f5a623"),
    brightBlue: ansi("bright-blue", "--lt-slate", "#8ca2c4"),
    brightMagenta: ansi("bright-magenta", "--lt-text-soft", "#b9c2d4"),
    brightCyan: ansi("bright-cyan", "--lt-muted", "#8791a6"),
    brightWhite: ansi("bright-white", "--lt-text", "#e7ecf5"),
  };
}
