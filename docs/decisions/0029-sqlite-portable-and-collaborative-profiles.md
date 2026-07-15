# ADR 0029: Define portable and collaborative SQLite profiles

## Status

Accepted.

## Decision

Portable SQLite remains uninstrumented and fully conventional, with external changes reconciled through schema inspection, row hashing where feasible, or snapshot replacement.

Collaborative SQLite adds documented `_lattice_*` metadata, stable primary keys, audit triggers, migration records, and sequence tracking so ordinary external writers can produce replicable row changes. Missing or bypassed instrumentation falls back to snapshot reconciliation.

Destructive schema changes occur through branches and serialized migration workflows.

## Consequences

Lattice is honest about the incompatibility between arbitrary uninstrumented external writes and guaranteed semantic replication. Users choose interoperability strength appropriate to the data application's collaboration needs.
