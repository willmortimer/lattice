# latticed

Long-lived Lattice daemon: Unix-domain control plane, optional semantic indexing,
and an authenticated **localhost-only** HTTP / MCP context API.

## Local HTTP API (D6)

Binds **`127.0.0.1` only** (never `0.0.0.0`). Default port: `18787`
(`--api-port 0` disables).

Authenticate every `/v1/*` call with the daemon instance token:

```http
Authorization: Bearer <token>
```

or

```http
X-Lattice-Token: <token>
```

| Method | Path | Purpose |
| --- | --- | --- |
| `GET` | `/health` | Liveness (no auth) |
| `POST` | `/v1/search` | Hybrid (default) or FTS search with provenance |
| `POST` | `/v1/read` | Bounded page/resource read by path |
| `POST` | `/v1/related` | Backlinks + FTS related stub |
| `POST` | `/v1/build_context` | Bounded excerpts; `export_policy=ask/deny` omitted or flagged |

Bodies accept `workspaceId` (open session) or `root` (opens a read session).
Payloads are capped (`maxBytes` / hit limits). Hybrid hits with
`export_policy` of `ask` or `deny` redact excerpts; `build_context` never
exfiltrates `ask` text freely (`needsConsent: true`).

This surface is **read-oriented**. Mutations continue through the semantic
command / Unix protocol path — the API is not a second write authority.

### Example

```sh
cargo run -p lattice-daemon -- --auth-token dev-token --api-port 18787

curl -s -X POST http://127.0.0.1:18787/v1/search \
  -H 'authorization: Bearer dev-token' \
  -H 'content-type: application/json' \
  -d '{"root":"/path/to/workspace","query":"notes","mode":"fts"}'
```

## MCP stdio

Minimal JSON-RPC MCP adapter exposing the same four tools:

```sh
LATTICE_AUTH_TOKEN=dev-token cargo run -p lattice-daemon -- mcp
```

Tools: `search`, `read`, `related`, `build_context`. Prefer the HTTP contract
for automated tests; use MCP when wiring Claude Desktop / other stdio clients.

Example Claude Desktop snippet:

```json
{
  "mcpServers": {
    "lattice": {
      "command": "latticed",
      "args": ["mcp"],
      "env": { "LATTICE_AUTH_TOKEN": "dev-token" }
    }
  }
}
```

## Tests

```sh
cargo test -p lattice-daemon
```
