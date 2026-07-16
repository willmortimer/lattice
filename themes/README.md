# Themes

Lattice themes are YAML documents. Built-ins live here; user themes go in
`~/Lattice/Settings/themes/*.theme.yaml`. Appearance prefs:
`~/Lattice/Settings/appearance.yaml`. Workspace accent (90% case):
`.lattice/theme.yaml`.

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
| `fonts` | `display`, `ui`, `mono` stacks |
| `shape` | radii, grid pitch, titlebar, max width |

Components consume only `--lt-*` CSS variables (roles + derived washes).
Themes must not inject arbitrary CSS.
