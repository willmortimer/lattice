# Lattice contributor guide

## Read this first

Lattice is a fast, local-first, open-native workspace. The specification in
`docs/` is intentionally broader than the current Phase 0–1 implementation.
Before making architectural or product-surface changes, read:

- `docs/01-product-vision.md`
- `docs/02-principles-and-invariants.md`
- `docs/04-system-architecture.md`
- `docs/23-frontend-rendering-and-performance.md`
- `docs/25-ux-capability-discovery-and-product-scope.md`
- `docs/29-roadmap.md`
- `docs/38-design-review-addendum.md`

For local voice dictation (FluidAudio / Parakeet, capture, editor provisional
text, `latticed` protocol), read `docs/voice/README.md` and
`docs/decisions/0040-local-voice-dictation-documentation.md` before changing
boundaries.

Use the ADRs in `docs/decisions/` for accepted decisions. New irreversible
choices should receive an ADR rather than silently becoming implementation
facts.

## Repository map

- `apps/cli/`: headless `lattice` CLI.
- `apps/desktop/`: Tauri 2 desktop shell using React, TypeScript, and Vite.
- `crates/`: Rust domain crates for resources, storage, commands, indexing,
  data applications, and themes.
- `themes/`: built-in YAML theme sources.
- `site/`: Astro marketing site and Starlight documentation.
- `design/`: algorithmic visual-identity source and logo lab.
- `docs/`: product, architecture, roadmap, and ADRs.

## Architectural invariants

- The workspace is a real directory; canonical content remains inspectable
  outside Lattice.
- Offline is the normal state. Do not add mandatory network dependencies to
  core workflows.
- Every mutation performed through Lattice must flow through the semantic
  command/transaction core. Frontend code must not become a privileged writer.
- External file edits remain legitimate and must be reconciled honestly.
- Rust owns canonical resource state, validation, storage, commands, search,
  data orchestration, and capability enforcement.
- React coordinates shell UI and lifecycle. It must not own editor, canvas,
  grid, chart, notebook, or other performance-critical hot loops.
- Load resources and capabilities lazily. Do not hydrate an entire workspace
  into frontend memory.
- Prefer bounded queries, cancellation, virtualization, and coarse state
  transfer. Large tabular or binary payloads should not become JSON object
  piles over Tauri IPC.
- Preserve progressive disclosure. The primary creation vocabulary is Page,
  Canvas, Table, Notebook, and File; deeper capabilities appear contextually.
- Advanced source, history, lineage, permissions, queries, logs, and conflicts
  belong under per-resource Inspect surfaces, not a separate global mode.

## Desktop frontend

- Keep shell state local or in small, fine-grained stores. Avoid a single
  monolithic global state object.
- Use accessible semantic DOM for shell controls and maintain full keyboard
  operation, visible focus, reduced-motion behavior, and screen-reader labels.
- Use Tiptap/ProseMirror transactions for page editing and PixiJS for canvas
  rendering. A production data grid must own scrolling/editing outside React's
  per-cell render loop.
- UI actions should call typed frontend adapters that invoke semantic Tauri
  commands. Avoid scattering raw `invoke()` calls through presentation
  components as the shell grows.
- Keep the browser-only desktop surface a demo fixture; it must not imply that
  browser mode has native filesystem authority.

## Themes and visual identity

- Theme YAML in `themes/*.theme.yaml` is the source of truth. Components consume
  semantic `--lt-*` variables only.
- Regenerate theme outputs with `pnpm compile-theme`; do not hand-edit:
  - `apps/desktop/src/theme-tokens.css`
  - `apps/desktop/src/theme-tokens.ts`
  - `site/src/styles/theme-tokens.css`
- Workspace template packages under `templates/workspaces/` are source files.
  Regenerate the embedded catalogs with `pnpm compile-templates`; do not
  hand-edit `crates/lattice-core/src/template_catalog.generated.rs`,
  `apps/desktop/src/templateCatalog.generated.ts`, or
  `apps/desktop/src/demoWorkspace.generated.ts`.
- The Lattice mark is generated from the axonometric unit-cell algorithm in
  `site/scripts/generate-mark.mjs`. Do not hand-edit generated mark geometry.
- Visuals should follow `design/philosophy.md`: structure is quiet, signal is
  warm and sparse, geometry is reproducible, and motion respects
  `prefers-reduced-motion`.

## Development and verification

Linux Dev Container / DevCell cell demos (browser UI + site + headless CLI, no
Tauri) use `.devcontainer/` and `scripts/devcontainer/` — see
`docs/dev/devcontainer.md`.
Nix remains the source of truth for the native desktop shell.

Prefer the documented Nix entry points:

```sh
nix run .#test
nix run .#lint
nix run .#check
nix run .#site-build
nix run .#desktop-build
```

Equivalent focused commands:

```sh
cargo test --workspace
pnpm --filter @lattice/desktop test
pnpm --filter @lattice/desktop build
pnpm --filter @lattice/site build
pnpm compile-theme
```

- Run commands from the repository root.
- Add or update tests for behavior changes, especially command preconditions,
  recovery, reconciliation, Markdown round trips, resource trees, data views,
  and theme compilation.
- Treat performance budgets as product requirements. Profile production builds
  before introducing heavier frontend abstractions.
- Do not edit generated documentation under `site/src/content/docs/` directly;
  update `docs/` and run the docs sync/build workflow.

## Change discipline

- Preserve unrelated work in a dirty tree.
- Keep changes scoped and reversible.
- Update documentation in the same change when behavior, formats, commands,
  environment variables, or architecture contracts change.
- Prefer a visible degraded fallback over hidden data loss or silent failure.
