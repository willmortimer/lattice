---
title: Architecture overview
description: How Lattice keeps a native local core, responsive specialized surfaces, and ordinary canonical files.
---

You do not need to understand the architecture to use Lattice. These boundaries
explain why the app can stay local-first, interoperable, and responsive as it
adds richer resources.

Lattice uses Tauri 2 for the native desktop shell.

- **Rust** owns canonical resource state, storage, validation, semantic
  commands, search, indexing, data orchestration, and capability enforcement.
- **React and TypeScript** coordinate the shell, navigation, dialogs, command
  discovery, lifecycle, and accessibility.
- **Specialized renderers** own hot loops: ProseMirror/Tiptap for pages, PixiJS
  for canvas, and a production grid engine for large tables.

The frontend loads bounded views lazily and sends coarse typed messages across
Tauri IPC. Large tabular or binary data does not become a pile of JSON objects
or per-cell React components.

The optional `latticed` service hosts longer-lived workspace sessions, file
watching, search, voice supervision, and local API/MCP access. The desktop can
still perform core interactive work without a remote server.

Read the full [system architecture](https://github.com/willmortimer/lattice/blob/main/docs/04-system-architecture.md)
and [frontend performance contract](https://github.com/willmortimer/lattice/blob/main/docs/23-frontend-rendering-and-performance.md).
