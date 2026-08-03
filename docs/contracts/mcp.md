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
| Optional cloud MCP gateway | [ ] Near |
| Rich UI tool result components for every tool | [ ] Near |

## Authority rules

1. Local workspace authority stays with the user machine and daemon policy.
2. Mutations from agents should land as **proposals** unless an explicit
   approved path says otherwise.
3. Do not treat MCP as a bypass around Inspect, history, or permissions.

## Typical client shape

```json
{
  "mcpServers": {
    "lattice": {
      "command": "latticed",
      "args": ["mcp"]
    }
  }
}
```

Exact binary names and install paths vary by packaging. Prefer the documented
desktop/daemon install for the build you are on.

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
