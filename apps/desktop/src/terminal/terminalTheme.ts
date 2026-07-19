import type { ITheme } from "@xterm/xterm";

function readToken(name: string, fallback: string): string {
  if (typeof document === "undefined") return fallback;
  const value = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return value || fallback;
}

/** Map Lattice `--lt-*` tokens to an xterm.js theme. */
export function latticeTerminalTheme(): ITheme {
  return {
    background: readToken("--lt-bg", "#0a0d13"),
    foreground: readToken("--lt-text", "#e7ecf5"),
    cursor: readToken("--lt-accent", "#f5a623"),
    cursorAccent: readToken("--lt-on-accent", "#201403"),
    selectionBackground: readToken("--lt-accent-wash", "rgba(245, 166, 35, 0.1)"),
    black: readToken("--lt-bg-raise", "#0e1320"),
    brightBlack: readToken("--lt-faint", "#5f6a80"),
    red: readToken("--lt-danger", "#ff9d8a"),
    green: readToken("--lt-accent-bright", "#ffce8a"),
    yellow: readToken("--lt-accent", "#f5a623"),
    blue: readToken("--lt-slate", "#8ca2c4"),
    magenta: readToken("--lt-text-soft", "#b9c2d4"),
    cyan: readToken("--lt-muted", "#8791a6"),
    white: readToken("--lt-text", "#e7ecf5"),
    brightRed: readToken("--lt-danger", "#ff9d8a"),
    brightGreen: readToken("--lt-accent-bright", "#ffce8a"),
    brightYellow: readToken("--lt-accent", "#f5a623"),
    brightBlue: readToken("--lt-slate", "#8ca2c4"),
    brightMagenta: readToken("--lt-text-soft", "#b9c2d4"),
    brightCyan: readToken("--lt-muted", "#8791a6"),
    brightWhite: readToken("--lt-text", "#e7ecf5"),
  };
}
