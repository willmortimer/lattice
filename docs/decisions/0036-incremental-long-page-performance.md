# ADR 0036: Incremental long-page performance before ProseMirror block virtualization

## Status

Accepted.

## Context

Product docs require long-page performance work: block virtualization for
extremely long pages, lazy media and embed rendering, and resource
suspension for offscreen content (`docs/07-markdown-code-and-documents.md`,
`docs/23-frontend-rendering-and-performance.md`, `docs/04-system-architecture.md`).
The page editor uses Tiptap/ProseMirror with React node views for images,
lattice embeds, and code blocks.

Full ProseMirror block virtualization — mounting only visible document
blocks while preserving selection, undo, collaborative mapping, and
Markdown round-trip — is a large, risky change. Nothing in that class is
implemented yet. The PDF viewer already uses bounded retention and
viewport-range helpers (`apps/desktop/src/viewers/media/pdfVirtualization.ts`).

## Decision

**Defer full block virtualization.** Ship incremental wins that defer
expensive preview work inside existing node views until the block is near
the viewport.

1. **Keep every ProseMirror node in the document model.** React node views
   remain mounted so block selection, drag handles, and editing semantics
   stay correct. We do not unmount node views based on scroll position in
   this phase.

2. **Defer heavy preview work with `IntersectionObserver`.** When a heavy
   embed is offscreen (with modest root margin overscan), skip:
   - workspace binary reads and Blob URL creation for inline images;
   - Mermaid diagram rendering under fenced `mermaid` code blocks.

   Show a lightweight placeholder with reserved minimum height (and
   explicit width/height when present on the node) until the block
   intersects the viewport.

3. **Treat lightweight embed chrome as eager.** `LatticeEmbedView` today
   only renders path metadata, not live resource previews. It stays eager
   until embed previews become expensive enough to warrant the same deferral.

4. **Follow PDF viewer discipline.** Use bounded overscan, cancel in-flight
   work on unmount or when scrolling away, and degrade to eager loading
   when `IntersectionObserver` is unavailable.

Full block virtualization remains a future milestone once incremental wins
are measured and editor invariants for virtual block lists are designed.

## Alternatives considered

- **Full ProseMirror block virtualization now** — highest long-term payoff
  but high risk to editing correctness, plugin compatibility, and
  round-trip tests; out of scope for the first slice.
- **Lazy-unmount entire React node views** — would break selection and
  drag-handle behavior for offscreen blocks without a much larger
  coordination layer.
- **Eager previews (status quo)** — simple but scales poorly on long pages
  with many images or Mermaid diagrams.

## Consequences

- Long pages with many images or Mermaid blocks avoid eager binary I/O and
  diagram layout for offscreen content.
- Users may briefly see placeholders while scrolling; placeholders reserve
  height to limit layout shift.
- Future heavy embed previews (PDF thumbnails, live data views) should
  reuse the same visibility-deferral hook rather than inventing per-type
  scroll listeners.
- Block virtualization, inactive-page cached renderers, and off-thread
  syntax highlighting remain open follow-ups tracked against performance
  budgets in `docs/23`.
