# ADR 0037: Localhost headless bridge shares handlers with Tauri

## Status

Accepted.

## Context

The desktop shell exposes workspace, page, search, and home operations
through Tauri `#[tauri::command]` handlers in `apps/desktop/src-tauri`.
A planned localhost HTTP bridge must offer the same MVP surface to the
browser-only desktop demo and other headless callers without depending on
Tauri IPC types (`Request`, `Response`, invoke bodies).

Duplicating handler logic in the bridge would drift from the desktop shell
and violate the invariant that every mutation flows through the semantic
command core with identical preconditions and error shapes.

## Decision

Extract tauri-free handler functions into `crates/lattice-handlers`. The
desktop shell keeps thin `#[tauri::command]` wrappers that delegate to
`lattice_handlers::*`. A future localhost HTTP server (single-tenant,
bound to `127.0.0.1`) will call the same functions and serialize the
existing DTOs (`WorkspaceSnapshot`, `PageContent`, `SearchHit`, etc.).

MVP extraction scope:

- workspace: `open_workspace`, `list_resources`
- pages: `read_page`, `apply_page_update`, `create_page`
- search: `search_workspace`, `rebuild_index`, `get_backlinks`
- home/provisioning: `ensure_home`, `list_templates`, `create_workspace`
- shared path containment and command-error formatting

Commands that require Tauri raw request bodies (binary resource updates,
asset import) remain in the desktop crate until the bridge defines an
equivalent transport boundary.

## Consequences

- Handler behavior is tested once in `lattice-handlers`; Tauri wrappers
  stay thin.
- JSON/DTO shapes remain serde-compatible with the existing React adapters.
- Non-MVP Tauri commands may continue to call domain crates directly until
  a later extraction pass.
- The bridge must not become a second write path: it calls handlers, not
  `Workspace` or `CommandEngine` directly.
