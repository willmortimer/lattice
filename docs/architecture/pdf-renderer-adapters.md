# PDF renderer adapters

The desktop PDF surface has a host-owned controller and a replaceable rendering
engine. This boundary exists so a future PDFium-backed candidate can be tested
without changing resource IPC, viewer interaction, or canonical PDF files.

## Current provider

`PdfJsRendererAdapter` is the only registered production provider. It owns all
PDF.js imports, worker setup, range transport, password/error normalization,
page cleanup, and worker teardown. `PdfViewer` owns toolbar controls, find UI,
zoom, viewport virtualization, the three-canvas budget, and degraded states.

The native `read_resource_range` command remains the source of truth. A
`PdfSource` first inspects the resource and then permits abortable reads no
larger than 256 KiB. PDF.js requests are split through the shared range
transport before calling that source.

## Contracts

- `PdfSource` exposes an identity, total byte length, and bounded range reads.
- `PdfRendererAdapter` declares an ID and capabilities, then opens a source.
- `PdfRendererSession` owns page lookup, optional find, and idempotent disposal.
- `PdfPageHandle` owns a page's dimensions, render task and cleanup. Page
  acquisition, sizing, and rendering receive an `AbortSignal`; eviction cancels
  the active render task before releasing its page.

Find is capability-gated because it is not required of future engines. The
viewer hides the find affordance when the selected adapter does not offer it.

## Selection and future engines

`createDefaultPdfRendererProvider()` selects PDF.js exclusively today. Future
adapters must preserve the range-backed, cancellable lifecycle; they must not
bypass capability enforcement or add a second resource reader. EmbedPDF and
PDFShell are intentionally not dependencies in this slice. A future provider
may select them only after their progressive local-source behavior and cleanup
semantics meet the same contracts and benchmark budget.
