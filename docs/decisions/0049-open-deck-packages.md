# ADR 0049: Use open Deck packages for portable presentations

## Status

Accepted.

## Context

Lattice needs a presentation-native resource without making a proprietary
binary deck its canonical format or forcing every deck to become a web app.
The source must remain readable by people, agents, ordinary editors, and
future exporters while allowing Lattice to provide a high-quality deck view.

## Decision

Use a `.deck/` directory with a `deck.yaml` manifest. Deck v1 contains one
semantic HTML source file per slide, optional package-local CSS, and optional
Markdown speaker notes. The manifest has a stable deck ID and stable unique
slide IDs, plus a `16:9` or `4:3` aspect ratio, ordered slides, optional start
slide, timer duration, loop setting, and host-owned `cut`, `fade`, or directed
`push` transitions.

The core validates all paths as package-relative and verifies referenced files
exist inside the package after symlink resolution. Unknown manifest fields are
ignored by the v1 parser so later optional capabilities do not make current
decks unreadable. Canonical sources are never structurally rewritten merely
because Lattice does not understand those fields.

## Consequences

- Decks are first-class resources while their content remains directly
  inspectable outside Lattice.
- Presentation UI, export, remote navigation, and composed resource viewboxes
  can evolve behind a resource/session adapter without changing canonical
  sources.
- Import/export adapters may target PowerPoint, Google Slides, PDF, or static
  HTML later, but none of those formats becomes the Deck source of truth.
