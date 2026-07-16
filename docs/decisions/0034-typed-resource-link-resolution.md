# ADR 0034: Resolve cross-resource links through one typed catalog

## Status

Accepted.

## Decision

Link parsing and resolution live in `lattice-core` and are shared by indexing,
template validation, and desktop navigation. Targets carry canonical link
text, display label, workspace-relative path, and resource kind.

Resolution is deterministic:

- Exact workspace-relative paths win.
- Markdown pages may omit `.md`.
- Folders require a trailing slash.
- Other resource kinds retain their identifying extension or package suffix.
- Relative Markdown links resolve from the source page.
- Unique basename fallback is allowed; ambiguous matches produce candidates
  and never select arbitrarily.
- Anchors are preserved.

The desktop runtime caches a bounded `ResourceCatalog`, refreshes it with the
workspace listing, and exposes typed search and resolution commands.

## Consequences

Autocomplete can show resource kinds and remain bounded. Folder links reveal
the tree, ambiguous links open a picker, missing page links can offer creation,
and backlinks use the same meaning as editor navigation.
