---
title: Search, history, and conflicts
description: Find resources, inspect backlinks and history, undo semantic changes, and handle external edits safely.
---

## Search and jump

- **⌘/Ctrl+K** opens workspace search.
- **⌘/Ctrl+P** opens the command palette for resources and actions.

Keyword search is local and available immediately. Optional semantic search is
disabled until you enable it in **Settings → Search**, accept the model download,
and let local indexing finish. Hybrid results indicate whether keyword,
semantic, or both signals matched.

## Inspect a resource

Open **Inspect** for the selected resource. Available sections depend on kind and
may include properties, backlinks, history, schema, source, permissions, and
diagnostics. Lattice exposes deeper behavior here instead of introducing a
separate expert mode.

## Undo and history

Press **⌘/Ctrl+Z** outside an active editor to undo the most recent compatible
semantic command. The CLI also provides `lattice history`, `lattice undo`, and
`lattice redo`.

Editor undo, command undo, and restoring an older materialized revision are
different operations. Inspect shows which history Lattice can safely reverse.

## Handle an external change

External editors are legitimate writers. When a watched resource changes,
Lattice waits for a stable materialization, reconciles the strongest safe
structure, and reloads or shows a conflict envelope.

If an external edit invalidates a pending command's precondition, do not force
the old command. Reload, compare the current materialization, and create a new
change or revision repair. This preserves both outside work and Lattice's
transaction history honestly.
