# ADR 0045: GitHub connected repos are read-only Extract mirrors

## Status

Accepted.

## Context

Lattice is local-first: the workspace directory is canonical
([ADR 0001](0001-native-filesystem-is-canonical.md)). Remote systems attach as
connectors with Live / Extract / Composite modes
([docs/12-remote-data-connectors-and-query.md](../12-remote-data-connectors-and-query.md)).
Git/GitHub is registered as a plugin capability
([docs/37-capability-and-format-registry.md](../37-capability-and-format-registry.md)).

Users want to connect a GitHub repository they own, browse its tree, and open
files without turning Lattice into an embedded IDE. A Live API-only tree would
fight offline defaults; dumping a working tree into Notes would blur authored
content with a remote mirror.

## Decision

1. **Auth** uses a Lattice **GitHub App** with user-to-server tokens via the
   **OAuth device flow**, driven from the **CLI** (`lattice github login` /
   `lattice github connect`). The desktop shell browses Connected extracts
   only and does not present connect/login UI. Long-lived PATs are not the
   product path. Tokens live in the OS keychain (or a test in-memory store);
   binding manifests hold keychain references only
   ([docs/20-security-permissions-secrets-and-trust.md](../20-security-permissions-secrets-and-trust.md)).

2. **Materialization** is **Extract**: `git clone --depth 1` into
   `.lattice/connectors/github/<binding_id>/checkout/`. Bindings are YAML under
   `.lattice/connectors/github/<binding_id>.yaml` with `mode: read` and
   capabilities `[list, read, snapshot]` (no `mutate`).

3. **Sandbox:** Extract paths are operational storage, not a second workspace
   root. Do not call `Workspace::open` on a checkout even if it contains
   `lattice.yaml`. Command-core mutations targeting `.lattice/` (including
   connector extracts) remain rejected. The desktop exposes a separate
   **Connected roots** tree and opens files with `editable: false`.

4. **Refresh** is fetch + hard reset to the remote default-branch tip. No push
   credentials are exposed to the UI in this slice.

5. **Permissions** for the initial App slice: Contents (read-only) and Metadata
   (read-only). Issues/PRs/history are deferred.

## Consequences

- Connected repos degrade offline by serving the last extract with a stale badge.
- A future write mode requires an explicit capability grant and ADR revision.
- Generic Phase 6 connector traits can absorb this crate; the GitHub slice may
  ship ahead of the full framework as a shaped spike.
