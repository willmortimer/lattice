# ADR 0026: Auto-approval is an explicit policy language

## Status

Accepted.

## Decision

Agent and automation auto-approval is governed by an inspectable policy language constraining actor, commands, paths, resource kinds, operation counts, deletion, schema changes, executable content, dependencies, network, remote writes, publishing, validation, recurrence, expiration, and authored-region ownership.

Capability grants, new secret access, new remote writes, private publication, destructive schema migration, permanent deletion, unsigned native code, and policy weakening remain nondelegable by default.

## Consequences

There is no unscoped “always allow this agent” setting. Policy evaluation, explanation, testing, versioning, and audit become part of the security architecture.
