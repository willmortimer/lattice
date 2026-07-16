---
title: Core concepts
description: The principles that make Lattice open-native and local-first.
---

## A workspace is a directory

Canonical content remains inspectable outside Lattice. Hidden state may improve
indexing, previews, recovery, and history, but deleting `.lattice/` must never
destroy the work itself.

## Offline is normal

Core editing, search, data, canvas, and command workflows do not require a
server. Connectivity is an optional capability.

## Every mutation is semantic

The desktop UI is not a privileged writer. GUI actions, the CLI, and future
automation surfaces use the same Rust command and transaction core.

## Different resources keep appropriate formats

Markdown should not impersonate a database, canvas, notebook, or binary file.
Lattice composes independent resources through links, views, commands, and
inspection instead of flattening everything into one storage model.

Read the canonical [principles and invariants](https://github.com/willmortimer/lattice/blob/main/docs/02-principles-and-invariants.md).
