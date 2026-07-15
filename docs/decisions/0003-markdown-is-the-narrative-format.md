# ADR 0003: Markdown is the narrative format, not the universal format

## Status
Accepted

## Context
Markdown is durable, human-readable, and well understood by people and agents, but cannot losslessly represent databases, canvases, sheets, notebooks, or full applications.

## Decision
Use conservative CommonMark/GFM Markdown with YAML front matter, optional wiki links and stable block IDs, Mermaid fences, and a small documented directive syntax for references to independent resources. Read additional dialects where practical, but write one predictable Lattice dialect.

## Consequences
- Narrative documents remain readable in generic editors.
- Rich resources retain their own formats.
- Some imported rich documents require structural rather than pixel-perfect conversion.
- Directives must always provide useful fallbacks.
