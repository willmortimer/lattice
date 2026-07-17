# ADR 0033: Use declarative templates and staged workspace provisioning

## Status

Accepted.

## Decision

Built-in workspace templates are packages under
`templates/workspaces/<id>/`. A versioned manifest declares presentation,
ordinary seed files, directories, optional declarative `dataPackages`,
capabilities, defaults, and visibility.
`pnpm compile-templates` validates packages and generates the embedded Rust
catalog, the TypeScript gallery catalog, and the browser-demo workspace
snapshot (`demoWorkspace.generated.ts`) from the `demo` sample package.

Creating a workspace defaults to a new named child directory. Lattice builds
and validates it in a unique sibling staging directory, then atomically renames
it into place. Initializing an existing directory is an explicit advanced
mode: preflight blocks every collision, existing files are never overwritten,
the workspace manifest is written last, and created paths are rolled back on
failure.

Provisioning success and default-workspace persistence are separate outcomes.
A workspace that exists is returned successfully even if making it the default
fails.

## Consequences

Template content has one source of truth and can later support external package
sources without changing provisioning semantics. Failed creation cannot expose
a half-built new workspace, and preference failures cannot misreport durable
workspace creation as a total failure.
