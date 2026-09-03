---
title: MCP contract
description: Public Model Context Protocol surface for Lattice workspaces.
---

# MCP contract

Lattice exposes workspace tools over MCP so external agents can inspect and
propose changes under the same authority model as the CLI and desktop app.

## Status

| Area | Status |
| --- | --- |
| Local stdio MCP via daemon (`latticed mcp`) | [x] Shipped |
| Read / search / schema / profile tools | [x] Shipped |
| Proposal creation and inspection | [x] Shipped |
| Shared executor with localhost API | [x] Shipped |
| Cloud MCP JSON-RPC + OAuth AS (DCR, PKCE, consent) | [x] Shipped (server) |
| Cursor / Claude Desktop stdio install (`latticed mcp --print-client-config`) | [x] Shipped |
| Cursor project install (`latticed mcp --install-cursor`) | [x] Shipped |
| Loopback HTTP MCP (`http://127.0.0.1:18787/mcp`) | [x] Shipped |
| In-app copy MCP config / cloud connector | [x] Shipped |
| Agent Plugins 1.0 package (`plugin.json` + `mcp.json`) | [x] Shipped |
| MCP Apps proposal UI (`text/html;profile=mcp-app`) | [x] Shipped |
| ChatGPT custom connector (cloud URL + OAuth DCR/PKCE/`iss`) | [x] Shipped (server) |
| Streamable HTTP GET `/mcp` SSE probe | [x] Shipped (probe only) |
| Claude Desktop / ChatGPT *store* listing | [ ] Near |
| Full Streamable HTTP GET session (server-initiated JSON-RPC) | [ ] Near |
| Lattice as a host of third-party Agent Plugins | [ ] Not planned |
| Rich UI tool result components for every tool | [ ] Near |

## Authority rules

1. Local workspace authority stays with the user machine and daemon policy.
2. Mutations from agents should land as **proposals** unless an explicit
   approved path says otherwise.
3. Do not treat MCP as a bypass around Inspect, history, or permissions.

## Typical client shape

Generate a ready-to-paste `mcpServers` block with the absolute path of the
`latticed` binary you invoked:

```sh
latticed mcp --print-client-config --client cursor
latticed mcp --print-client-config --client claude-desktop
latticed mcp --install-cursor
```

`--client` is required with `--print-client-config` (`cursor` or `claude-desktop`).
The JSON `command` is the absolute `latticed` path. `env.LATTICE_AUTH_TOKEN` and
`env.LATTICE_WORKSPACE_ROOT` are included only if those env vars are already set
in your shell. These commands print or write JSON and do **not** start the MCP
server.

`--install-cursor` merges a `"lattice"` stdio server into `./.cursor/mcp.json`
(create or replace that entry; other servers are preserved). Pass
`--cursor-config PATH` to write elsewhere. The JSON `env` block includes
`LATTICE_CLOUD_SESSION_FILE` pointing at
`~/Lattice/State/cloud-session` (or `%USERPROFILE%\Lattice\State\cloud-session`)
so Cursor's `latticed mcp` process can use the desktop cloud sign-in without
pasting a token. Sign in under **Settings → Cloud account** in Lattice.app
first (cloud-authoritative pages need that session).

**Cursor:** use `--install-cursor` in the project you want to drive, or merge
`--print-client-config` output into project `.cursor/mcp.json` or Cursor user
MCP settings. Then call `workspace.list` (or set `LATTICE_WORKSPACE_ROOT`)
before search/read/propose.

**Claude Desktop:** merge into `claude_desktop_config.json` (macOS:
`~/Library/Application Support/Claude/claude_desktop_config.json`; Windows:
`%APPDATA%\Claude\claude_desktop_config.json`).

This is **stdio install wiring**, not a packaged connector from the Claude
Desktop extension store or a DXT/mcpb bundle.

### Loopback HTTP

While `latticed` is running, MCP is also at **`http://127.0.0.1:18787/mcp`**
(default port; `--api-port` changes it). The socket is **`127.0.0.1` only**,
never `0.0.0.0`. Authenticate POSTs with `Authorization: Bearer <daemon token>`
(or `X-Lattice-Token`). Print the URL without starting stdio:

```sh
latticed mcp --print-loopback-url
```

`GET /mcp` with `Accept: text/event-stream` returns an empty SSE comment
(`: connected`) so Streamable HTTP clients can probe the endpoint. It is **not**
a full MCP session stream (no server-initiated JSON-RPC on GET). Other GET
Accept values return 405.

Desktop: **Settings → Plugins → Copy loopback URL**.

Local tools that need a workspace accept `workspaceId` or `root`. If both are
omitted, the daemon uses `LATTICE_WORKSPACE_ROOT` when that env var is set.
Call **`workspace.list`** first to see registered device workspaces.
Hosts that rewrite `.` to `_` (`workspace_list`) are accepted as aliases.

Local stdio and loopback MCP run as the **owner agent**: they can read the same
`ask` / `private` page bodies as the in-app agent. `secret` is still forbidden.
HTTP `/v1/read` and cloud MCP device relay stay on export policy (`ask`/`deny`
redact). Sign in on the desktop so cloud-authoritative reads can GET from
Lattice Cloud.

### Cloud connector

Cloud MCP JSON-RPC is **`https://cloud.lattice-notes.com/mcp`**. OAuth 2.1
discovery (DCR + PKCE). Authorize redirects include RFC 9207 `iss`. ChatGPT
custom connector callbacks
(`https://chatgpt.com/connector_platform_oauth_redirect` and
`https://chatgpt.com/connector/oauth/{id}`) are allowlisted, as are Claude
MCP callbacks and loopback (`http://127.0.0.1` / `localhost` / `[::1]`).

- `https://cloud.lattice-notes.com/.well-known/oauth-authorization-server`
- `https://cloud.lattice-notes.com/.well-known/oauth-protected-resource`
- `https://cloud.lattice-notes.com/.well-known/oauth-protected-resource/mcp`

Unauthenticated `tools/call` returns **401** with
`WWW-Authenticate: Bearer … resource_metadata="…/oauth-protected-resource/mcp"`
(RFC 9728) so ChatGPT custom connectors can start OAuth.

**ChatGPT / Codex custom connector:** paste `https://cloud.lattice-notes.com/mcp`
as the MCP server URL. The client should discover OAuth (401 `resource_metadata`
or well-known), register (DCR), and complete PKCE in the browser. Device-authority
tools (search/read/propose) need a paired online device advertising that
workspace; `workspace.list` and `workspace.get_lattice_docs` run on the cloud
without a device. Cloud MCP keeps export redaction (`ask`/`private` empty);
set `export_policy: allow` on pages you want ChatGPT to inspect.

Desktop: **Settings → Plugins → Copy cloud URLs** / **Copy cloud JSON**. Do not
paste access tokens into client config files.

### Agent Plugins 1.0

Lattice **exports** an [Agent Plugins 1.0](https://agent-plugins.org/) package
so Cursor, Codex, ChatGPT, and other plugin *hosts* can install Lattice. Lattice
does **not** load third-party Agent Plugins into the desktop app.

One-click export writes `plugin.json`, `mcp.json`, and `skills/…/SKILL.md`:

- Desktop: **Settings → Plugins → Save Agent Plugin…**
- CLI: `latticed mcp --print-agent-plugin --plugin-out DIR` (`--plugin-target local|cloud|both`)

`lattice.mcp` is local stdio plus documented loopback HTTP.
`lattice.mcp.cloud` is Streamable HTTP to Lattice Cloud (OAuth via well-known
metadata). This is not ChatGPT Apps SDK / skybridge and not a Claude Desktop
store listing.

### MCP Apps

Local and cloud MCP advertise `apps/list` and a proposal-review UI resource
(`ui://lattice/apps/proposal`, MIME `text/html;profile=mcp-app`). Proposal
tool results include `_meta.ui.resourceUri`. Hosts that do not embed Apps still
receive structured JSON.

## Tool categories (public)

| Category | Intent |
| --- | --- |
| Read | Fetch pages, records, schemas, bounded samples |
| Search | Full-text / structured find within the open workspace |
| Propose | Create reviewable change proposals |
| Inspect proposals | List / get proposal payloads |
| Docs | `workspace.get_lattice_docs` returns public contract Markdown |
| List | `workspace.list` — local: device registry; cloud: cloud workspaces |

Tool names evolve; clients should discover the live tool list from the MCP
server rather than hard-coding a private inventory.

## Docs resources (public contracts)

When the server advertises the MCP **resources** capability, public docs are
also listed as read-only URIs:

| URI | Content |
| --- | --- |
| `lattice://docs/index` | Topic index |
| `lattice://docs/cli` | CLI contract |
| `lattice://docs/mcp` | This MCP contract |
| `lattice://docs/api` | HTTP API contract |
| `lattice://docs/formats` | Format support matrix |
| `lattice://docs/integrations` | Integrations matrix |

Open-format folder layouts live on the docs site ([`/open/`](/open/)), not as
MCP resources.

Clients that under-use resources can call **`workspace.get_lattice_docs`** with
the same topic ids (or an empty/`list` topic for the catalog). Same Markdown
either way.
Online mirrors: [`/open/`](/open/), [`llms.txt`](/llms.txt),
[`llms-full.txt`](/llms-full.txt).

## Related

- [CLI contract](/docs/cli-contract/)
- [HTTP API contract](/docs/api/)
- [Integrations](/docs/integrations/)
