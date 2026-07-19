# lattice-bridge

Localhost HTTP server that exposes the MVP [`lattice-handlers`](../../crates/lattice-handlers)
surface to the browser demo (Vite on port 5173) without Tauri.

**Production / AI automation path:** use [`latticed`](../daemon)'s authenticated
localhost API (`127.0.0.1`, bearer token) or `latticed mcp` — not this bridge.
The bridge is a single-tenant demo fixture and must not be treated as a second
write authority or as proof that browser mode has native filesystem authority.

See [ADR 0037](../../docs/decisions/0037-localhost-bridge-shares-handlers-with-tauri.md).

## Run

```sh
cargo run -p lattice-bridge -- --port 8787

# Optional default workspace (omits `root` in request bodies):
cargo run -p lattice-bridge -- --root /path/to/workspace

# Optional auth (also via LATTICE_BRIDGE_TOKEN):
cargo run -p lattice-bridge -- --auth-token secret
```

Flags:

| Flag | Default | Description |
| --- | --- | --- |
| `--host` | `127.0.0.1` | Bind address (keep loopback) |
| `--port` | `8787` | Listen port |
| `--root` | _(none)_ | Default workspace root |
| `--auth-token` / `LATTICE_BRIDGE_TOKEN` | _(none)_ | When set, require `Authorization: Bearer` or `X-Lattice-Token` |

## CORS

CORS allows only Vite loopback origins (`http://localhost:5173` and
`http://127.0.0.1:5173`). This is a local debug aid — do not widen for general
cross-origin use.

## API

All handler routes use `POST` with JSON bodies (`camelCase` keys, matching the
React Tauri adapters). Success responses are JSON; errors return
`{ "error": "..." }` with HTTP 400 (409 for stale page revisions).

| Method | Path | Body | Success |
| --- | --- | --- | --- |
| `GET` | `/health` | — | `{ "status": "ok" }` |
| `POST` | `/open_workspace` | `{ "path" }` | `WorkspaceSnapshot` |
| `POST` | `/list_resources` | `{ "root"? }` | `Resource[]` |
| `POST` | `/read_page` | `{ "root"?, "relPath" }` | `PageContent` |
| `POST` | `/apply_page_update` | `{ "root"?, "relPath", "content", "baseRevision" }` | `{ "revision" }` |
| `POST` | `/create_page` | `{ "root"?, "relPath", "content"?, "templatePath"?, "title"? }` | `{ "revision" }` |
| `POST` | `/search_workspace` | `{ "root"?, "query", "limit"? }` | `SearchHit[]` |
| `POST` | `/rebuild_index` | `{ "root"? }` | `{ "pagesIndexed" }` |
| `POST` | `/get_backlinks` | `{ "root"?, "relPath" }` | `Backlink[]` |
| `POST` | `/ensure_home` | `{}` | `LatticeHomeInfo` |
| `POST` | `/list_templates` | `{}` | `TemplateDescriptor[]` |
| `POST` | `/create_workspace` | `{ "path", "title"?, "template", "setDefault"?, "initializeExisting"? }` | `WorkspaceProvisionResult` |

When `--root` is set, `root` may be omitted on workspace-scoped routes.

## Smoke test

```sh
cargo run -p lattice-bridge -- --port 8787 &
BRIDGE_PID=$!

curl -s http://127.0.0.1:8787/health

# After seeding a workspace (see docs/dev/devcontainer.md):
curl -s -X POST http://127.0.0.1:8787/open_workspace \
  -H 'content-type: application/json' \
  -d '{"path":"/path/to/workspace"}'

kill $BRIDGE_PID
```

## Tests

```sh
cargo test -p lattice-bridge
cargo build -p lattice-bridge
```
