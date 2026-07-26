# ADR 0047: Font packs own type stacks; themes reference them

## Status

Accepted.

## Context

Every built-in theme previously inlined identical `fonts:` stacks (display /
ui / mono). Only Cupertino diverged, using system SF faces. That made font
exploration expensive (edit 30+ YAML files) and conflated color themes with
typography.

Desktop already applies fonts only through `--lt-font-*` CSS variables.
Components must not branch on theme or pack ids.

## Decision

1. Ship first-class **font packs** as YAML under `themes/font-packs/*.font-pack.yaml`
   (user packs: `~/Lattice/Settings/font-packs/`).
2. Each pack defines `display`, `ui`, and `mono` CSS font-family stacks.
3. Theme documents reference a default pack via required `font_pack: <id>`
   instead of inlining stacks. Cupertino’s default is `apple`; others default
   to `lattice`.
4. Appearance settings may override the active pack (`fontPack: theme | <id>`)
   for A/B testing without editing themes. Resolution order:
   appearance override → theme default → builtin `lattice`.
5. Runtime flatten (Rust) and `scripts/compile-theme.mjs` resolve the pack
   into `--lt-font-*` (and `--l-font-*` aliases). Components still consume
   only those variables.
6. Bundled webfont faces for shipped packs load eagerly so pack switching is
   instant; the Apple pack uses system faces only.

## Consequences

- Color themes stay orthogonal to typography; authoring a new look is one pack
  file plus optional appearance override.
- New packs that need webfonts must add `@fontsource` (or equivalent) deps and
  imports on desktop and site builds.
- User themes must set `font_pack`; unknown pack ids fail at resolve/flatten
  with a clear diagnostic (fallback to `lattice` only for missing *themes*).
