# ADR 0008: AI is an interchangeable external client

## Status
Accepted

## Context
AI-oriented knowledge products often make a proprietary model, hidden memory system, or autonomous organizer central to the workspace.

## Decision
Lattice optimizes resources for human readability and exposes files, CLI, API, MCP, bounded context operations, and semantic transactions. It does not require a bundled agent, invisible canonical knowledge graph, or hosted model. Optional AI clients and plugins use the same public surfaces as other tools.

## Consequences
- Users can choose providers or use no model.
- AI-generated changes remain inspectable and reversible.
- Lattice still supports optional derived indexes, suggestions, and provenance without making them canonical.
