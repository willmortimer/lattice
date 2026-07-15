# ADR 0007: Every important mutation uses the semantic command core

## Status
Accepted

## Context
GUI-only behavior forces agents and scripts to automate interfaces or patch implementation details. Direct untracked writes weaken undo, history, validation, sync, and security.

## Decision
Desktop UI, CLI, local API, MCP, plugins, workflows, scripts, and agents share semantic commands and atomic transactions. Commands support preconditions, validation, previews, idempotency, history, and undo. Direct file access remains an open escape hatch and is reconciled by the watcher.

## Consequences
- Automation is a first-class capability rather than a premium integration.
- The command schema becomes a major compatibility contract.
- File reconciliation must produce semantic external revisions where possible.
