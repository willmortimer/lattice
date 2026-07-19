# Themes

Lattice themes are YAML documents. Built-ins live here; user themes go in
`~/Lattice/Settings/themes/*.theme.yaml`. Appearance prefs:
`~/Lattice/Settings/appearance.yaml`. Workspace accent (90% case):
`.lattice/theme.yaml`.

Built-ins fall into three groups:

- **Lattice originals** — `lattice-slate` (default dark), `lattice-paper`
  (default light), plus carbon, fjord, ultraviolet, blueprint, vellum,
  ember, moss, midnight, copper, rosewood, graphite (dark) and glacier,
  sandstone, orchid, meadow (light).
- **Platform looks** — `cupertino` (macOS idiom: SF stacks, system blue),
  `lattice-oled` (true `#000000` ground for AMOLED/OLED panels).
- **Adopted terminal standards** — `catppuccin-mocha`, `nord`,
  `github-dark`, `dracula`, `solarized-dark`. These carry a `terminal:`
  block with their canonical ANSI palettes (see below).

## Compile (site + static desktop tokens)

Prefer the packaged entrypoints — not a bare `node` invocation:

```sh
pnpm compile-theme
# or
nix run .#compile-theme
# or inside the dev shell
lattice-compile-theme
```

Writes:

- `apps/desktop/src/theme-tokens.css`
- `apps/desktop/src/theme-tokens.ts` (Pixi/canvas mirror)
- `site/src/styles/theme-tokens.css`

Desktop and site `predev` / `prebuild` hooks run the compiler automatically.

## Runtime (desktop)

- Loader applies `--lt-*` vars to `:root`; mirror in `localStorage` wins first paint.
- Auto appearance mode stores separate dark/light mirror variants so first paint
  follows `prefers-color-scheme` instead of the last session’s theme only.
- Startup shows a branded splash (mark + wordmark) for about a second by default;
  toggle under Settings → Workspaces & startup → Startup splash.
- Command palette: `Theme: Lattice Slate`, `Theme: Lattice Paper`, `Theme: Follow system`.
- CLI: `lattice theme list|check|set|mode`.
- Editing a user theme or `appearance.yaml` live-reloads the UI.
- Optional workspace override: `.lattice/theme.yaml` with `theme:` and/or `accent:`.

## Schema (v0)

| Key | Meaning |
| --- | --- |
| `name` / `id` | Human label and stable id |
| `appearance` | `dark` \| `light` → CSS `color-scheme` |
| `palette` | Raw named colors |
| `roles` | Semantic tokens (`$paletteKey` refs or literals) |
| `terminal` | Optional ANSI palette (see below) |
| `fonts` | `display`, `ui`, `mono` stacks |
| `shape` | radii, grid pitch, titlebar, max width |

Components consume only `--lt-*` CSS variables (roles + derived washes).
Themes must not inject arbitrary CSS.

## Terminal palettes (`terminal:`)

Themes adopted from terminal-theme standards keep their real 16-color ANSI
palettes instead of the role-derived approximation:

```yaml
terminal:
  black: $surface1     # all 16 ANSI slots required when the block exists
  red: $red
  # … green yellow blue magenta cyan white + bright_* variants
  cursor: $rosewater   # optional
  cursor_text: $base   # optional
  selection: "#585b7066"  # optional; literal hex/#RRGGBBAA (xterm can't
                          # parse color-mix)
```

Slots flatten to `--lt-term-*` vars (`bright_black` → `--lt-term-bright-black`).
`terminalTheme.ts` prefers these and falls back to role-derived colors for
themes without the block, so plain themes need nothing new.

## Wide gamut / HDR

Shipped built-ins are sRGB hex on purpose — see
`docs/dev/hdr-edr-color.md` for the Display-P3 / EDR feasibility report.
