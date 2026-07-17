# ADR 0035: Keep File kinds coarse and derive ResourceFormatProfile

## Status

Accepted.

## Context

Phase 1 must open pages, canvases, data apps, and ordinary files—including
images, PDFs, JSON, YAML, and source code—without multiplying top-level
`ResourceKind` variants for every extension. The shell also needs stable
renderer routing and bounded native inspection that never writes to the
workspace.

## Decision

Keep `ResourceKind::File` (and other coarse kinds) stable. Derive a
`ResourceFormatProfile` from extension, bounded magic-byte probe, and
lightweight structure validation. Expose capabilities (`can_read_range`,
`can_update`, `validates_structure`, and so on) from the profile rather
than encoding behavior in the kind enum.

Ordinary files are format-first: the desktop maps profiles to renderer format
IDs (`file:pdf`, `file:image`, …) and the native runtime enforces read
budgets independently of React.

## Consequences

New file types can gain profiles and renderers without changing the resource
tree model. Read-only binaries use metadata revisions; editable text within
the edit budget uses content hashes. Conformance fixtures and performance
budgets attach to profiles, not kinds. Irreversible kind proliferation is
avoided; profile additions remain explicit and testable.
