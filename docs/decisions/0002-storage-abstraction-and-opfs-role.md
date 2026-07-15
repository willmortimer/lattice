# ADR 0002: Use a storage abstraction without hiding real paths

## Status
Accepted

## Context
Lattice needs native, OPFS, memory, overlay, and remote snapshot stores while preserving ordinary resource paths and filesystem semantics.

## Decision
Implement a Rust `WorkspaceStore` abstraction for reading, listing, watching, atomic writing, metadata, rename, and removal. Native storage is the reference implementation. OPFS is used for browser working copies, caches, scratch resources, and offline mirrors. Memory and overlay stores support tests, previews, and proposed transactions.

## Consequences
- The same command system can target several environments.
- The abstraction cannot erase filesystem-specific behavior.
- Resource identity uses both stable IDs and human-readable relative paths.
