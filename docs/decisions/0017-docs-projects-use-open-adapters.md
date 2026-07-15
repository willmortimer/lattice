# ADR 0017: Documentation sites are first-class projects built through open adapters

## Status
Accepted

## Context
Lattice can provide a polished docs workflow without inventing a proprietary publishing language or generator.

## Decision
A docs project is a folder of normal Markdown and resources plus `docs.lattice.yaml`. Lattice provides navigation, validation, code references, generated reference integration, previews, and publishing. Astro Starlight is the likely default adapter, with VitePress, Docusaurus, mdBook, MkDocs, Quarto, Pandoc, and specialized reference generators supported.

## Consequences
- Existing documentation ecosystems remain usable.
- Lattice adds coherent project UX and validation.
- Generator-specific features may not round-trip perfectly into the common project model.
