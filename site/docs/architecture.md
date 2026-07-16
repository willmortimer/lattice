---
title: Architecture
description: The performance-oriented boundary between Rust, Tauri, React, and specialized renderers.
---

Lattice uses Tauri 2 for the native desktop shell.

- **Rust** owns canonical resource state, storage, validation, semantic
  commands, search, indexing, data orchestration, and capability enforcement.
- **React and TypeScript** coordinate the shell, navigation, dialogs, command
  discovery, lifecycle, and accessibility.
- **Specialized renderers** own hot loops: ProseMirror/Tiptap for pages, PixiJS
  for canvas, and a production grid engine for large tables.

The frontend loads bounded views lazily and sends coarse typed messages across
Tauri IPC. Large tabular or binary data should not become piles of JSON objects
or per-cell React components.

Read the full [system architecture](https://github.com/willmortimer/lattice/blob/main/docs/04-system-architecture.md)
and [frontend performance contract](https://github.com/willmortimer/lattice/blob/main/docs/23-frontend-rendering-and-performance.md).
