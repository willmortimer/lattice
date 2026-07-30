# ADR 0053: Lattice media asset model

## Status

Accepted (implementation phased; R1 locks still-capture defaults).

## Context

Capture produces pixels; the workspace needs a durable, inspectable model for
stills, annotations, timed media, and export renditions. Without a locked asset
shape, screenshot work will store ad hoc PNG files, fork formats per surface, and
push transcoding into one-off export scripts. The universal capture engine
([ADR 0052](0052-universal-capture-engine.md)) needs a single downstream contract
for files written to Capture Inbox, clipboard compat layers, and later recording
bundles.

## Decision

### LatticeAsset structure

A **LatticeAsset** packages:

1. **Immutable source** — lossless or canonical-by-policy bytes plus capture
   metadata (timestamp, source kind, display layout, optional source ref).
2. **Optional recipe** — non-destructive transforms, vector annotations, and
   timeline edits (crop, markup layers, trim, concat). Recipes are versioned;
   sources are not rewritten when a recipe changes.
3. **Renditions** — derived outputs keyed by purpose (`clipboard`, `preview`,
   `thumb`, `export-mp4`, etc.) with explicit codec, dimensions, and lineage
   back to source + recipe revision.

Workspace File resources reference LatticeAsset packages (directory or manifest
convention defined in implementation plans). Renditions remain rebuildable
derived state ([ADR 0022](0022-derived-resources-have-lineage.md)).

### Default still formats

| Role | Format | Rationale |
| --- | --- | --- |
| Canonical still on disk | **Lossless WebP** | Smaller than PNG at lossless settings; single still default |
| Clipboard / legacy compat | **PNG** | Pasteboard and external tool expectations |
| Everyday default | WebP + PNG renditions | Not AVIF or JPEG XL for routine capture |

AVIF and JPEG XL may appear as optional export renditions later; they are **not**
everyday capture defaults — decoder variance and tooling friction outweigh
marginal size wins for quick capture.

### Tiled capture for huge surfaces

Full-page, renderer, and Cell surfaces may exceed single-image limits. When height
or width exceeds safe encoder bounds (**WebP dimension limit 16,383 px**), capture
produces a **tiled LatticeAsset**: ordered tiles with shared metadata describing
logical canvas size, tile grid, and overlap policy. Preview and markup layers
compose tiles; export recipes may flatten to a single rendition when within limits
or produce tiled renditions when not.

### Still markup stack

Non-destructive still markup (R2) uses:

- **Konva** for vector annotation layers (arrows, boxes, text, blur regions).
- **Cropper.js** acceptable for rectangular crop UI where Konva is awkward.
- **jSquash** in **Web Workers** for decode/encode of still renditions (tier 1).
- **SVG** import/export and rasterization via **resvg** / **usvg** (tier 3).

Markup edits update the **recipe**; source tiles or source still bytes stay immutable
unless the user explicitly commits a destructive flatten.

### Timed media is separate

GIF, animated WebP, screen recordings, and camera clips are **timed media**, not
still LatticeAssets. They share the source + recipe + renditions pattern but use
a timeline-first recipe and distinct preview shell.

Processing escalation:

- **Mediabunny** first for browser-local trim, concat, and lightweight transcode
  (tier 4).
- **FFmpeg** and **Cell-hosted** pipelines for heavy transcode, batch export, and
  formats Mediabunny does not own (tier 5).

Recording project bundles from
([ADR 0052](0052-universal-capture-engine.md)) deserialize into timed-media
LatticeAssets; burned MP4 is a rendition, not the canonical save.

### Media execution tiers (0–5)

Media work escalates through fixed tiers so the shell does not pull FFmpeg into
quick screenshot paths:

| Tier | Capability | Typical stack |
| --- | --- | --- |
| **0** | Write source bytes; serve existing renditions | Native capture + filesystem |
| **1** | Still decode/encode in workers | jSquash workers |
| **2** | Interactive still markup | Konva (+ Cropper.js for crop UI) |
| **3** | SVG rasterization and vector hygiene | resvg / usvg |
| **4** | Timed media edit preview and light transcode | Mediabunny |
| **5** | Heavy transcode, batch export, Cell offload | FFmpeg, Cell execution |

UI and commands declare the **minimum tier** required; hosts refuse silently
degrading exports rather than blocking capture. Tier 0–1 suffice for R1; higher
tiers unlock per train in [capture roadmap](../architecture/capture-and-media.md).

### Clipboard and Capture Inbox

R1 clipboard paste uses the PNG rendition (tier 0/1). Capture Inbox stores the
canonical lossless WebP source (and tile manifest when tiled). PNG may be
omitted on disk if clipboard is skipped for a given capture.

## Consequences

- Screenshot and recording features share one asset mental model instead of
  parallel file conventions.
- Large captures remain representable without abandoning WebP as the default
  still codec.
- Markup and timed-media features add recipe and tier complexity without
  rewriting R1 inbox files.
- Preview shell, browser capture, and agentic capture (R3–R6) extend renditions
  and tiers rather than replacing LatticeAsset.
