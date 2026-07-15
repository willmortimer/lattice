# ADR 0010: A small automation and scheduling kernel belongs in core

## Status
Accepted

## Context
Leaving scheduling, hooks, and events entirely to plugins would recreate brittle script piles and inconsistent lifecycle behavior.

## Decision
Core provides typed semantic events, manual commands, validators, transaction transforms, post-commit subscribers, durable scheduled jobs, workflow execution, logs, and an optional local daemon. Workflows and tasks are open files. Slow or unreliable work runs after canonical commits unless explicitly modeled as a bounded validator.

## Consequences
- Common automation is coherent and observable.
- Lattice avoids implementing a domain-specific Zapier clone.
- Capability packs and plugins extend triggers and actions through public contracts.
