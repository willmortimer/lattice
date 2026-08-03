# Security, Permissions, Secrets, and Trust

## Threat model

Lattice may execute or render:

- Generated HTML and React applications.
- Third-party plugins.
- Python and notebook code.
- Shell commands.
- Workflows.
- Remote database queries.
- MCP clients.
- External websites.

The workspace may contain sensitive documents and credentials. Therefore no extension receives ambient authority.

## Capability model

Capabilities are scoped by actor, workspace, resource paths, operation, network host, secret, and duration.

```yaml
permissions:
  workspace:
    read:
      - Research/**
    write:
      - Research/Generated/**
  datasets:
    query:
      - Analytics/Usage.dataset/**
    mutate: []
    schema: []
  network:
    hosts:
      - api.crossref.org
  secrets:
    - crossref-api-key
  processes: []
```

Grant modes:

- This action.
- This session.
- This workspace.
- Permanent.
- Read-only.
- Proposed transaction only.

## Actor classes

- Trusted Lattice core.
- Bundled capability.
- Signed third-party plugin.
- Workspace script/task.
- Artifact.
- Lattice App.
- External embedded website.
- MCP client.
- Remote connector.
- Remote execution worker.

Each has a different default trust profile.

## WebView isolation

Artifacts and Apps use:

- Separate origin or isolated WebView.
- Strict CSP.
- No Tauri IPC unless a narrow bridge is explicitly assigned.
- Host allowlists.
- Read bindings instead of filesystem access.
- Proposed writes.
- Lifecycle suspension/destruction.
- Dependency and source inspection.

## Plugin isolation

Backend plugins prefer WASI components with typed host interfaces and resource limits. Native plugins are privileged and visibly identified.

## Script isolation

- Out-of-process execution.
- Declared working directory.
- Environment allowlist.
- Network policy.
- Timeout and memory limits.
- No inherited secrets by default.
- Structured input/output.
- Proposed transaction preference.

Containers or Nix may provide stronger reproducibility but do not automatically provide a security sandbox.

## MCP security

Each MCP client receives:

- Named identity.
- Workspace scope.
- Read/create/update/delete distinctions.
- Dataset query/mutate/schema distinctions.
- Artifact execution permissions.
- Transaction-size limits.
- Audit history.

Remote MCP uses OAuth/OIDC and short-lived tokens.

## Secrets

Supported providers:

- OS keychain.
- Environment references.
- 1Password or other provider plugins.
- OIDC/OAuth token stores.
- SSH agent.
- Self-hosted secret manager.

Manifests contain references, not secret values.

```yaml
credentials:
  provider: keychain
  key: production-postgres-readonly
```

### OS credential store (desktop production)

OAuth, cloud bearer, and BYO API keys persist through the Rust
[`keyring`](https://github.com/hwchen/keyring-rs) crate. On Windows this maps to
**Credential Manager** (Generic credentials); on macOS to Keychain (App Group
SecItem when the desktop bundle has entitlements, otherwise legacy keyring); on
Linux to Secret Service when available.

Handlers call [`production_token_store`](../../crates/lattice-connectors/src/credentials/mod.rs)
once at startup. A dedicated **probe** account (never the user-session key) tests
write/delete; failure falls back to in-process [`MemoryTokenStore`](../../crates/lattice-connectors/src/credentials/mod.rs)
for CI/sandbox only.

| Service (target) | Account (user name) | Purpose |
| --- | --- | --- |
| `lattice.github` | `lattice.github.user` | GitHub OAuth user token |
| `lattice.github` | `lattice.github.{binding_id}` | Per-repo binding token |
| `lattice.github` | `lattice.github.probe` | Startup keyring probe (ephemeral) |
| `lattice.gitlab` | `lattice.gitlab.user` | GitLab OAuth user token |
| `lattice.gitlab` | `lattice.gitlab.{binding_id}` | Per-project binding token |
| `lattice.gitlab` | `lattice.gitlab.probe` | Startup keyring probe (ephemeral) |
| `lattice.cloud` | `lattice.cloud.user` | Lattice cloud bearer session |
| `lattice.cloud` | `lattice.cloud.probe` | Startup keyring probe (ephemeral) |
| `lattice.ai.openai` | `api-key` | BYO OpenAI API key (desktop) |

Additional connector providers use `lattice.{provider}` / `lattice.{provider}.user`
via `token_service_for` / `user_token_key_for` in `lattice-connectors`.

Values are JSON-serialized [`TokenMaterial`](../../crates/lattice-connectors/src/credentials/mod.rs)
(access token, optional refresh, expiry, type).

**Windows manual smoke** (release build, interactive session):

1. Sign in to Lattice cloud (or connect GitHub/GitLab) once.
2. Open **Credential Manager → Windows Credentials** and confirm a Generic
   credential whose target matches `lattice.cloud` / `lattice.github` / etc.
3. Quit and relaunch the desktop app; session should restore without re-auth.
4. Run `cargo test -p lattice-connectors keychain_round_trip_when_writable`
   on the same machine to confirm the probe passes (skips when keyring is
   unavailable, e.g. headless CI).

See also [environment.md](dev/environment.md).

## Remote databases

- Read-only default.
- Parameterized queries.
- Statement timeout.
- Cancellation.
- Row/byte limits.
- Explain before expensive execution where possible.
- Explicit write and schema permissions.
- Visual production warning.
- Destructive SQL confirmation.

## Supply chain

- Lockfiles retained.
- Plugin/app package signatures where available.
- Dependency inventory and SBOM support.
- Hash-pinned container images and remote tools.
- Reproducible build metadata.
- Vulnerability scanning hooks.
- Untrusted package inspection before installation.

## Workspace trust

Opening an unknown workspace should default to safe mode:

- No automatic scripts.
- No workflow schedules.
- No automatic notebook execution (user-initiated Pyodide Run is explicit).
- No app builds.
- No network access beyond approved capability surfaces (Pyodide CDN load on Run).
- No plugin activation without review.

The user can trust specific capabilities rather than the entire directory indiscriminately.

## Telemetry privacy

OpenTelemetry spans must not include document bodies, SQL result values, secrets, or arbitrary filenames by default. External telemetry export is opt-in or explicitly configured.

## Encryption

Local encryption policy is primarily delegated to platform storage, full-disk encryption, and optional encrypted workspace providers. Cloud sync may support end-to-end encrypted opaque blobs for personal mode, with managed team mode offering server-side indexing only when intentionally enabled.

Optional **workspace encryption** (DEK wrapping, biometric-gated key access) remains an open design track; see [ADR 0038](decisions/0038-workspace-encryption-os-keychain-and-local-authentication.md).

## App lock (session privacy)

The desktop shell may enable **app lock** (Settings → Privacy) on macOS:

- Touch ID or device password unlocks the session via LocalAuthentication in Rust.
- While locked, the UI shows a privacy overlay and privileged Tauri IPC returns `app-locked`.
- Files on disk stay ordinary workspace content; app lock is not at-rest encryption.
- The same `request_user_presence` helper is the intended hook for future privileged-action approvals.

See [ADR 0049](decisions/0049-app-lock-session-privacy-local-authentication.md).

## Audit

Record:

- Actor.
- Command and transaction.
- Resources affected.
- Permission grant.
- Secrets referenced, not values.
- Remote hosts contacted.
- Execution environment.
- Result and failure.

Audit records are user-accessible and exportable.
