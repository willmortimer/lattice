---
title: ADR 0001 Record architecture decisions
---

# ADR 0001: Record architecture decisions

## Status

Accepted

## Context

Engineering choices accumulate quickly: framework boundaries, storage formats, API shapes,
and operational trade-offs. Without a durable record, rationale is lost and teams
re-litigate the same questions.

## Decision

We will capture significant technical choices as lightweight architecture decision
records in [[Decisions/]]. Each ADR includes context, the decision, and consequences.
Superseded ADRs move to [[Archive/]] with a link to the replacement.

## Consequences

- Decisions stay discoverable from [[Home]] and [[Architecture/System Overview.canvas]].
- New ADRs follow the same heading structure for consistency.
- Minor implementation details stay in code comments or [[Debug Journal/]] instead.
