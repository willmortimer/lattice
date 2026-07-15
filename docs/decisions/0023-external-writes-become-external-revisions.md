# ADR 0023: External writes become external revisions

## Status

Accepted.

## Decision

Every mutation performed through Lattice uses the semantic command core. Mutations performed by external tools become first-class external revisions. Format adapters synthesize semantic differences where safe and fall back to opaque replacement revisions where they cannot.

Editor undo, command undo, and revision reversion are distinct operations. External changes may invalidate command undo and require three-way reversion.

## Consequences

External tools remain legitimate writers without forcing Lattice to invent semantic history that does not exist. Reconciliation becomes a major subsystem with per-format adapters, proposal invalidation, and explicit recovery behavior.
