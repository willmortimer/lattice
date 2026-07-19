# ADR 0006: Deterministic voice commands

## Status

Accepted (voice subsystem).

## Context

Natural-language command interpretation is attractive but unsafe and hard to
test for an initial release that also inserts free-form prose.

## Decision

Initial formatting and slash-command support uses **explicit modes** and a
**deterministic grammar** (dictation reserved words, command mode, or mixed
mode with an explicit wake prefix). No implicit NL command detection in v1.

## Consequences

- Behavior is predictable and testable.
- Ordinary prose is less likely to trigger actions.
- Natural-language command interpretation is deferred.
- See [voice-commands.md](../voice-commands.md).
