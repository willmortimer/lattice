# ADR 0019: Capabilities are lazy, contextual, and workspace-scoped

## Status
Accepted

## Context
Lattice aims to support many formats and workflows without recreating Notion's clutter, memory use, and permanently visible product surface.

## Decision
Keep a small core shell. Ship official capabilities as lazy-loaded removable modules, and domain systems as capability packs. Each workspace records enabled capabilities. Slash commands, menus, background services, indexers, and settings are contextual.

## Consequences
- Breadth does not require ambient complexity.
- Capability activation, migration, and dependency handling become explicit.
- A fresh workspace remains approachable.
