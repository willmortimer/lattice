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
```

`--client` is required (`cursor` or `claude-desktop`). The JSON `command` is the
absolute `latticed` path. `env.LATTICE_AUTH_TOKEN` is included only if that env
var is already set in your shell. This command prints JSON to stdout and does
**not** start the MCP server.

**Cursor:** merge the output into project `.cursor/mcp.json` or Cursor user MCP
settings.

**Claude Desktop:** merge into `claude_desktop_config.json` (macOS:
`~/Library/Application Support/Claude/claude_desktop_config.json`; Windows:
`%APPDATA%\Claude\claude_desktop_config.json`).

This is **stdio install wiring**, not a packaged connector from the Claude
Desktop extension store or a DXT/mcpb bundle.

## Tool categories (public)

| Category | Intent |
| --- | --- |
| Read | Fetch pages, records, schemas, bounded samples |
| Search | Full-text / structured find within the open workspace |
| Propose | Create reviewable change proposals |
| Inspect proposals | List / get proposal payloads |
| Docs | `workspace.get_lattice_docs` returns public contract / open-format Markdown |

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
| `lattice://docs/open/{page,…}` | Open format folder layouts |

Clients that under-use resources can call **`workspace.get_lattice_docs`** with
the same topic ids (or an empty/`list` topic for the catalog). Same Markdown
either way.
Online mirrors: [`/open/`](/open/), [`llms.txt`](/llms.txt),
[`llms-full.txt`](/llms-full.txt).

## Related

- [CLI contract](/docs/cli-contract/)
- [HTTP API contract](/docs/api/)
- [Integrations](/docs/integrations/)
