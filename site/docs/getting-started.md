---
title: Getting started
description: Create or open a Lattice workspace and understand what appears on disk.
---

Lattice workspaces are normal directories marked by a readable
`lattice.yaml`. You can create the default `~/Lattice` home, create a workspace
in another folder, or open an existing workspace.

## The desktop workflow

1. Open Lattice and create or choose a workspace.
2. Use the Files sidebar to open pages, canvases, tables, and ordinary files.
3. Create a page with the visible `+` menu or open Quick Capture with
   `Cmd/Ctrl+N`.
4. Use search with `Cmd/Ctrl+K` and the command palette with `Cmd/Ctrl+P`.
5. Open Inspect for properties, backlinks, history, schema, source, permissions,
   and diagnostics.

Pages are Markdown files. Lattice autosaves through its semantic command core
and reports a conflict instead of silently overwriting an external edit.

## Interoperate with other tools

Canonical files remain available to Finder, Explorer, terminals, Git, backup
software, VS Code, and format-specific applications. Quick Note and resource
actions can open the same file externally; Lattice does not require a separate
export step.
