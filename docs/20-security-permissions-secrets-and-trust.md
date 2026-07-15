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
- No notebook execution.
- No app builds.
- No network access.
- No plugin activation without review.

The user can trust specific capabilities rather than the entire directory indiscriminately.

## Telemetry privacy

OpenTelemetry spans must not include document bodies, SQL result values, secrets, or arbitrary filenames by default. External telemetry export is opt-in or explicitly configured.

## Encryption

Local encryption policy is primarily delegated to platform storage, full-disk encryption, and optional encrypted workspace providers. Cloud sync may support end-to-end encrypted opaque blobs for personal mode, with managed team mode offering server-side indexing only when intentionally enabled.

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
