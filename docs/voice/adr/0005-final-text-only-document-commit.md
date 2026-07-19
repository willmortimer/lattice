# ADR 0005: Final text only enters document state

## Status

Accepted (voice subsystem).

## Context

Committing partial ASR hypotheses into Markdown storage, undo, CRDT, indexing,
or sync would create noisy history and collaboration hazards.

## Decision

Provisional transcripts remain **local decorations** (or equivalent transient
composition) and **never** enter storage, undo, CRDT, indexing, automation, or
synchronization. Only final transcripts produce editor transactions through the
semantic command path.

## Consequences

- Collaboration receives stable text only.
- Editor integration requires a transient composition layer.
- Crashes during dictation may lose the provisional utterance unless recovery
  buffering is added (Quick Note recovery stores final text, not provisional).
- See [editor-integration.md](../editor-integration.md).
