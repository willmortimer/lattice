# ADR 0001: The native filesystem is canonical

## Status
Accepted

## Context
Lattice promises ordinary files, Git compatibility, external-editor access, shell automation, direct agent access, and useful workspaces outside the application. OPFS and application databases are performant but hidden behind an application origin or internal schema.

## Decision
Desktop and mobile workspaces use user-visible native directories as canonical storage. Active documents may be buffered in memory, and `.lattice/` may contain disposable indexes, recovery journals, caches, and sync state. Browser clients may use OPFS as a working mirror but must support synchronization and export.

## Consequences
- Files remain usable without Lattice.
- External edits are supported and reconciled.
- Atomic writes and recovery require deliberate engineering.
- Browser storage follows a different implementation path without changing the logical workspace contract.
