# Frontend, Rendering, and Performance

## Frontend decision

Use React 19 with TypeScript and Vite for the shell, while keeping specialized rendering and state machines out of React hot paths.

React wins on ecosystem integration, contributor availability, editor/chart/notebook tooling, and AI-generated application familiarity. Solid and Svelte remain credible alternatives, and the architecture should keep the shell replaceable.

## Rendering ownership

```text
React shell            windows, menus, inspectors, lifecycle
ProseMirror/Tiptap      document editing
PixiJS/WebGPU/WebGL     canvas scene and large geometry
Custom grid             mutable SQLite editing
Perspective             analytical grid and pivoting
Jupyter client          cell state and kernel execution
Vega/ECharts/Plotly     chart state
PencilKit               active iPad ink
Sandboxed WebViews      artifacts and apps
```

React coordinates; it does not mediate per-frame or per-keystroke internals.

## State model

- Local React state for ephemeral shell UI.
- Fine-grained external store for resource summaries and cross-surface state.
- ProseMirror transactions for pages.
- Imperative scene graph for canvas.
- Rust core for canonical resource state.
- Arrow buffers for tabular results.
- Typed semantic events between surfaces.

TanStack Store or a small custom signal store is a candidate. Avoid one giant global Redux-style object.

## Quick-note performance

Separate entry bundle:

```text
workspace.tsx
quick-note.tsx
artifact-host.ts
```

Quick note loads only shell, one editor, save bridge, theme, and basic open/search. No DuckDB, canvas, Jupyter, charting, or plugin host before typing.

## Performance budgets

Initial targets:

- Warm shell visible in 300–500 ms on representative hardware.
- Quick-note editor interactive as close to native-editor latency as practical.
- Keystroke-to-paint within one frame under normal load.
- Indexed search first results under 50 ms target.
- Canvas pan/zoom at 60 fps under benchmark scene sizes.
- Table scrolling smooth with virtualization.
- Workspace open time independent of total resource count.
- Inactive resources release substantial memory.

## Mandatory optimizations

- Lazy capability loading.
- Resource suspension states.
- Virtualized pages, tables, and canvases.
- Incremental parsing/indexing/sync/builds.
- Bounded queries.
- Native and columnar data paths.
- Worker-based parsing and layout.
- Preview caches.
- Cancellation and backpressure.

## Resource suspension

```text
active                full editor and live bindings
visible inactive      lightweight renderer
nearby/offscreen      cached preview
closed                serialized session state
unloaded              no frontend code resident
```

Jupyter kernels may hibernate or shut down separately from notebook rendering.

## Web Workers and OffscreenCanvas

Worker candidates:

- Markdown parsing.
- Mermaid and Graphviz layout.
- PDF rendering.
- Thumbnail generation.
- Search tokenization.
- Data profiling.
- Chart transforms.
- Canvas edge routing.
- Ink simplification.
- App compilation.

OffscreenCanvas can support preview and scene work outside the UI thread where WebView support permits.

## WebGPU

Design for optional WebGPU with WebGL fallback.

High-value uses:

- Huge canvases.
- Millions of chart marks.
- Geospatial layers.
- Graph rendering.
- Ink tessellation.
- Heatmaps and image filters.
- GPU picking.
- Certain local compute shaders.

Do not use WebGPU for normal text, forms, or small tables.

## WASM

Appropriate for:

- Portable plugins.
- Browser-only SQLite/DuckDB/Pyodide.
- Parsers and codecs.
- Sandboxed transforms.
- Shared Rust libraries in future web client.
- Geometry algorithms.

Native Rust remains preferable on desktop for trusted core work.

## Large IPC

Avoid JSON over Tauri IPC for:

- Arrow tables.
- PDF pixels.
- Notebook binary outputs.
- Canvas snapshots.
- Video/image buffers.

Use:

- Transferable ArrayBuffers.
- Streaming custom protocol.
- Arrow IPC.
- Shared buffers where safely supported.
- Memory-mapped cache files.

## Multiple WebViews

Use separate WebViews for substantial untrusted or heavy surfaces:

- Artifact/app.
- External web embed.
- PDF renderer where useful.
- Notebook output host.

Do not create one WebView per small canvas node.

## Native `wgpu` escape hatch

If profiling proves system WebView graphics insufficient, Lattice may compose a native Rust `wgpu` surface alongside the WebView for giant canvas or advanced ink. This is a stretch path because it adds platform and input complexity.

## Benchmarking

Maintain benchmark workspaces for:

- Many small pages.
- Extremely long pages.
- Huge canvases.
- Many active embeds.
- Million-row SQLite data apps.
- Multi-gigabyte Parquet.
- Notebook-heavy projects.
- PDF and media libraries.
- Plugin and workflow load.

Performance work is driven by profiles, not novelty.
