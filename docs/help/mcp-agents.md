---
title: Connect an agent
description: Wire Cursor, ChatGPT, or Codex to Lattice MCP — stdio, loopback, or cloud OAuth.
---

# Connect an agent

Lattice is an **MCP server**. Cursor, Codex, ChatGPT, and other Agent Plugins
1.0 **hosts** install Lattice. Lattice does not load other people's plugins.

Use **Settings → Plugins** (gear on the activity rail) or the CLI below.

## Test from Cursor (this checkout)

From the project you want the agent to edit:

```sh
latticed mcp --install-cursor
```

That merges a `"lattice"` stdio server into `.cursor/mcp.json`. Reload MCP in
Cursor. Sign in under **Settings → Cloud account** in Lattice first so the MCP
process can read cloud-backed pages (it uses the shared session file, not a
token in `mcp.json`). In a chat, call `workspace.list` first (or set
`LATTICE_WORKSPACE_ROOT` to a registered workspace folder). Writes create
**proposals** only — they do not apply.

Local Cursor MCP can read `ask`-policy pages the way the in-app agent can.
ChatGPT's cloud connector still redacts those pages.

Optional: `LATTICE_AUTH_TOKEN` and `LATTICE_WORKSPACE_ROOT` in the environment
are copied into the JSON `env` block when you run `--install-cursor` or
`--print-client-config`.

## What to copy (Settings → Plugins)

| Control | What it does |
| --- | --- |
| **Copy stdio JSON** | `latticed mcp` block for Cursor `mcp.json` or Claude Desktop |
| **Copy loopback URL** | `http://127.0.0.1:18787/mcp` for a running daemon (token required on POST) |
| **Copy cloud URLs** | `https://cloud.lattice-notes.com/mcp` plus OAuth well-known links |
| **Copy cloud JSON** | Remote `mcpServers` object with the cloud URL |
| **Save Agent Plugin…** | Writes Agent Plugins 1.0 folders (`plugin.json` + `mcp.json` + skill) |

Local HTTP MCP always uses **127.0.0.1**, not `0.0.0.0`. Cloud copy-paste never
includes access tokens — the client signs in with OAuth.

## ChatGPT / Codex custom connector

Paste **`https://cloud.lattice-notes.com/mcp`** as the connector URL. The client
discovers OAuth, registers (DCR), and completes PKCE in the browser. Search and
propose still need a paired online device for that workspace.
`workspace.list` and `workspace.get_lattice_docs` work on the cloud without a
device. Pages with `export_policy: ask` stay empty on this connector; use
`allow` for pages you want ChatGPT to read.

## Agent Plugins 1.0

CLI: `latticed mcp --print-agent-plugin --plugin-out DIR`. Point a host that
already supports [Agent Plugins 1.0](https://agent-plugins.org/) at that folder.
This is not ChatGPT Apps SDK / skybridge and not a Claude Desktop store listing.

This is not a Claude Desktop store listing. Workspace chat is still **Show
agent** on the activity rail; see [Workspace agent](agent.md).
