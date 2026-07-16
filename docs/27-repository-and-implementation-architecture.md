# Repository and Implementation Architecture

## Monorepo outline

```text
lattice/
├── apps/
│   ├── desktop/
│   ├── mobile/
│   ├── web/
│   ├── quick-note/
│   └── server-admin/
├── crates/
│   ├── core/
│   ├── format/
│   ├── storage/
│   ├── recovery/
│   ├── commands/
│   ├── transactions/
│   ├── history/
│   ├── search/
│   ├── catalog/
│   ├── canvas/
│   ├── ink/
│   ├── datasets/
│   ├── duckdb/
│   ├── arrow-transport/
│   ├── connectors/
│   ├── jupyter/
│   ├── tasks/
│   ├── workflows/
│   ├── plugins/
│   ├── apps-runtime/
│   ├── mcp/
│   ├── api/
│   ├── sync/
│   ├── security/
│   ├── secrets/
│   ├── telemetry/
│   └── publishing/
├── packages/
│   ├── shell-ui/
│   ├── editor/
│   ├── canvas-ui/
│   ├── data-grid/
│   ├── notebook-ui/
│   ├── diagnostics-ui/
│   ├── ui-kit/
│   ├── app-sdk/
│   ├── plugin-sdk/
│   ├── format-js/
│   └── mcp-client/
├── services/
│   ├── syncd/
│   ├── workerd/
│   ├── publishd/
│   └── telemetryd/
├── plugins/
│   ├── bundled/
│   └── examples/
├── specifications/
│   ├── workspace/
│   ├── canvas-profile/
│   ├── ink/
│   ├── data-app/
│   ├── artifact/
│   ├── app/
│   ├── workflow/
│   ├── plugin-api/
│   └── sync-protocol/
├── conformance/
├── benchmarks/
└── docs/
```

## Rust crate boundaries

Core domain crates avoid Tauri dependencies. Desktop, daemon, CLI, and server compose them.

Important interfaces:

- `WorkspaceStore`.
- `ResourceProvider`.
- `ParserSerializer`.
- `CommandHandler`.
- `TransactionParticipant`.
- `DataSource`.
- `ArrowStreamProvider`.
- `RendererDescriptor`.
- `TaskRuntime`.
- `WorkflowAction`.
- `PluginHost`.
- `Connector`.
- `SyncBackend`.
- `SecretProvider`.
- `Publisher`.

## Frontend packages

The frontend shell should not import every capability eagerly. Use route and resource-based dynamic loading.

- `shell-ui`: windows, tabs, inspectors, command palette.
- `editor`: ProseMirror/Tiptap and Markdown serializer.
- `canvas-ui`: Pixi scene and DOM overlays.
- `data-grid`: mutable SQLite table UI.
- `notebook-ui`: Jupyter client.
- `ui-kit`: application components and tokens.
- `app-sdk`: isolated app bridge.

## Native mobile plugins

- PencilKit and Apple Pencil interactions.
- Platform file access.
- Share sheet and quick capture.
- Background task integration.
- Secure keychain.
- Android stylus integration later.

## Build system

- Cargo workspace.
- pnpm workspace.
- Vite frontend builds.
- Tauri application packaging.
- `uv` for Python environments and test fixtures.
- Optional Nix flake for reproducible development.
- OCI images for server and workers.

## Generated code

Workspace template manifests and seed files under `templates/workspaces/` are
the source of truth. Run `pnpm compile-templates` after changing them. It
validates paths, collisions, bounds, sources, and seeded links, then writes:

- `crates/lattice-core/src/template_catalog.generated.rs`
- `apps/desktop/src/templateCatalog.generated.ts`

Do not edit either generated catalog directly.

Use generation for:

- Rust/TypeScript format models from schemas.
- WIT bindings.
- OpenAPI clients.
- MCP schemas.
- SQL migrations metadata.
- Test fixtures.

Generated output is checked or reproducibly rebuilt according to repository policy.

## Feature flags

Compile-time features should not fragment user format compatibility. Runtime capabilities and plugins handle most optional behavior.

## Development profiles

- Minimal desktop shell.
- Full bundled capabilities.
- Server.
- Mobile.
- Browser/OPFS experiment.
- Conformance-only.
- Benchmark/profile.

## Documentation

Architecture docs, ADRs, specifications, schemas, and conformance examples live in the repository and can be published through Lattice's own docs-project capability.
