# Themes

Lattice themes are YAML documents. Built-ins live here; user themes go in
`~/Lattice/Settings/themes/*.theme.yaml`. Appearance prefs:
`~/Lattice/Settings/appearance.yaml`. Workspace accent (90% case):
`.lattice/theme.yaml`.

Built-ins fall into three groups:

- **Lattice originals** — `lattice-slate` (default dark), `lattice-paper`
  (default light), plus carbon, fjord, ultraviolet, blueprint, ember, moss,
  midnight, copper, rosewood, graphite, solar flare, tidepool, `lattice-dusk`
  (dim mid-tone ground for long sessions), and `lattice-monochrome` (greyscale
  accent — signal from value, not hue) (dark), and vellum, glacier, sandstone,
  orchid, meadow, porcelain, matcha, limestone, and `lattice-daylight`
  (high-contrast white ground) (light).
- **Platform looks** — `cupertino` (macOS idiom: system blue, default
  `font_pack: apple`), `lattice-oled` (true `#000000` ground for AMOLED/OLED
  panels).
- **Terminal-derived palettes** — `catppuccin-mocha`, `nord`,
  `github-dark`, `dracula`, `solarized-dark`, `tokyo-night`, `gruvbox-dark`,
  `one-dark`, `rose-pine-moon`, `kanagawa-wave`, `everforest-dark`, and
  `ayu-dark`. These carry a `terminal:` block with an ANSI palette adapted
  from the corresponding terminal theme (see below).

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
| `font_pack` | Id of a pack under `themes/font-packs/` (see below) |
| `shape` | radii, grid pitch, titlebar, max width |

Components consume only `--lt-*` CSS variables (roles + derived washes).
Themes must not inject arbitrary CSS.

## Font packs

Typography is orthogonal to color themes (ADR 0047). Packs live in
`themes/font-packs/*.font-pack.yaml` (user packs:
`~/Lattice/Settings/font-packs/`).

| Pack | Display | UI | Mono |
| --- | --- | --- | --- |
| `lattice` | Fraunces | Space Grotesk | JetBrains Mono |
| `apple` | SF Pro Display | SF Pro Text | SF Mono |
| `atelier` | Literata | Source Sans 3 | JetBrains Mono |
| `signal` | Newsreader | IBM Plex Sans | JetBrains Mono |

Themes set `font_pack: <id>`. Appearance settings may override with
`fontPack: theme | <id>` (Settings → Appearance). Resolution: appearance
override → theme default → builtin `lattice`.

## Terminal palettes (`terminal:`)

Themes derived from terminal palettes keep an explicit 16-color ANSI family
instead of the role-derived approximation:

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
