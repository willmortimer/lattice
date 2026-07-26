# ADR 0045: GitHub/GitLab connected repos are read-only Extract mirrors

## Status

Accepted.

## Context

Lattice is local-first: the workspace directory is canonical
([ADR 0001](0001-native-filesystem-is-canonical.md)). Remote systems attach as
connectors with Live / Extract / Composite modes
([docs/12-remote-data-connectors-and-query.md](../12-remote-data-connectors-and-query.md)).
Git/GitHub/GitLab is registered as a plugin capability
([docs/37-capability-and-format-registry.md](../37-capability-and-format-registry.md)).

Users want to connect a repository they own, browse its tree, and open files
without turning Lattice into an embedded IDE. A Live API-only tree would fight
offline defaults; dumping a working tree into Notes would blur authored content
with a remote mirror.

## Decision

1. **Shared desktop OAuth** is provider-agnostic authorization-code + PKCE:
   - **AuthPresenter** opens the authorize URL in the **system browser**
     (`tauri-plugin-opener`). Embedded webviews are not the default (IdP
     policy and password-manager / passkey quality).
   - **Redirect modes:**
     - **Loopback** `http://127.0.0.1:17872/callback` when the IdP requires
       http(s) (GitHub App).
     - **Custom scheme** `lattice://oauth/callback` when allowed (GitLab),
       completed via `tauri-plugin-deep-link` → `oauth_ingest_callback`.
   - Tokens live in the OS keychain (or a test in-memory store); binding
     manifests hold keychain references only
     ([docs/20-security-permissions-secrets-and-trust.md](../20-security-permissions-secrets-and-trust.md)).

2. **GitHub** uses a Lattice **GitHub App**:
   - Desktop: loopback OAuth (`LATTICE_GITHUB_APP_CLIENT_ID` +
     `LATTICE_GITHUB_APP_CLIENT_SECRET`).
   - CLI: **device flow** (`lattice github login` / `connect`); client id only.
   - Extract under `.lattice/connectors/github/<id>/`.

3. **GitLab** uses a GitLab **OAuth application**:
   - Desktop: custom-scheme OAuth (`LATTICE_GITLAB_OAUTH_CLIENT_ID` +
     `LATTICE_GITLAB_OAUTH_CLIENT_SECRET`); scopes `read_api read_repository`.
   - CLI: same authorization-code flow with **loopback** (`lattice gitlab login`
     prints the URL and waits). Register both redirect URIs on the GitLab app.
   - Extract under `.lattice/connectors/gitlab/<id>/`.

4. **Materialization** is **Extract**: `git clone --depth 1` into
   `.lattice/connectors/<provider>/<binding_id>/checkout/`. Bindings are YAML
   with `mode: read` and capabilities `[list, read, snapshot]` (no `mutate`).

5. **Sandbox:** Extract paths are operational storage, not a second workspace
   root. Do not call `Workspace::open` on a checkout even if it contains
   `lattice.yaml`. Command-core mutations targeting `.lattice/` remain rejected.
   The desktop **Connected** tree opens files with `editable: false`.

6. **Refresh** is fetch + hard reset to the remote default-branch tip. No push
   credentials are exposed to the UI in this slice.

7. **Permissions** for the initial slice: read-only contents/metadata. Issues /
   MRs / history are deferred.

## Consequences

- Connected repos degrade offline by serving the last extract with a stale badge.
- A future write mode requires an explicit capability grant and ADR revision.
- GitHub App callback: `http://127.0.0.1:17872/callback`.
- GitLab OAuth callbacks: `lattice://oauth/callback` and
  `http://127.0.0.1:17872/callback`.
- Generic Phase 6 connector traits can absorb this crate; GitHub/GitLab ship as
  shaped spikes ahead of the full framework.
