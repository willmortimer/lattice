# ADR 0004: Start with JSON Canvas and a Lattice profile

## Status
Accepted

## Context
Lattice needs an infinite canvas that composes pages, data views, notebooks, artifacts, files, ink, and nested canvases without swallowing their contents into an opaque scene document.

## Decision
Use JSON Canvas as the interoperable spatial skeleton. Store richer renderer selection, reading order, responsive layout, permissions, bindings, and print behavior in a documented sidecar profile. Substantial content remains in independent resources referenced by file nodes.

## Consequences
- Basic canvases remain openable by other JSON Canvas tools.
- Lattice can add richer behavior without forking canonical content.
- A future documented Lattice canvas superset may be introduced if the base format becomes constraining.
- Compiled scene indexes and thumbnails remain disposable caches.
