---
title: Workspace agent
description: Chat with the agent panel beside your files.
---

# Workspace agent

The **Workspace agent** is a chat panel for questions, drafts, and proposals
about the workspace you have open.

## Open the agent

Click the **robot** button on the left **activity rail**. The label toggles
between **Show agent** and **Hide agent**.

The panel header shows **Workspace agent** and the current thread title.

## Layout

Open the layout menu in the header (it shows the current mode, such as
**Dock**):

| Mode | What it does |
| --- | --- |
| **Dock** | Compact sidebar beside your files |
| **Workbench** | Split conversation and evidence panes |
| **Focus** | Agent fills this window (press Escape to leave Focus) |
| **Detached** | Agent in a separate window |

Pick the layout that matches how much screen you want for chat vs editing.

## Threads

Click the thread title in the header to open the thread list:

- **Search threads** filters by title.
- **New** (plus) starts a fresh conversation for this workspace.
- Click a thread to switch to it. Pinned threads show a pin icon.
- Open **⋯** on a thread for **Rename**, **Pin** / **Unpin**, **Archive**, or
  **Delete**. Archive hides the thread from the list. Delete asks for
  confirmation and removes the thread and its messages.

While the agent is running, **Stop** appears in the header. Thread switching is
paused until the run finishes.

## Settings

Model and API options live under **Settings → AI** (gear on the activity rail).
Sign in under **Settings → Cloud account** when a cloud-backed provider
requires it.

To connect Cursor or Claude Desktop, open **Settings → Plugins** and copy MCP
config, the loopback URL, or the cloud connector. See
[Connect an agent](mcp-agents.md).
