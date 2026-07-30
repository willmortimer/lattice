# Capture and media roadmap

**Status:** Accepted sequencing for universal capture and Lattice media assets  
**Branch in flight:** `feat/clipper-t1` (train R1 only)  
**Last updated:** July 29, 2026

This document sequences capture and media work across release trains. Contracts
are locked in:

- [ADR 0052: Universal capture engine](../decisions/0052-universal-capture-engine.md)
- [ADR 0053: Lattice media asset model](../decisions/0053-lattice-media-asset-model.md)

Related:

- [Design review addendum §8.2](../38-design-review-addendum.md#82-move-capture-earlier)
- [Voice ADR 0008: Native client capture](../voice/adr/0008-native-client-capture.md)

---

## Trains

### R1 — Screenshot foundation (`feat/clipper-t1`) — **in scope**

**Goal:** Credible macOS still capture into clipboard + Capture Inbox with native
hot paths and Lattice UI exclusion.

**Scope:**

- `lattice-capture-core` + macOS ScreenCaptureKit adapter
  (`SCShareableContent`, `SCContentFilter`, `SCScreenshotManager`).
- Sources: `Display`, `Window`, `Region` with frozen multi-monitor overlay.
- Destinations: default clipboard + Capture Inbox; optional insert hooks stubbed.
- LatticeAsset v1: lossless WebP source, PNG clipboard rendition; tiled manifest
  when dimensions exceed WebP limits.
- Global shortcut / menu entry invoking native session (not WebView overlay).
- Recording API stub returning project-bundle shape (no recorder impl).

**Manual smoke:** [capture-smoke.md](../dev/capture-smoke.md) (build, permissions,
⌘⇧2, clipboard + inbox, cancel, multi-monitor). CI runs unit tests only.

**Out of scope for R1:** markup editor, browser tab capture, real recording,
Mediabunny, agent capture tools, media preview shell maturity.

---

### R2 — Still markup + insert — **not in `feat/clipper`**

**Goal:** Annotate and crop captures, then insert into the focused note or canvas.

**Scope:**

- Konva markup layers + Cropper.js crop UI (recipe updates, tier 2).
- jSquash worker renditions (tier 1) for preview sizes.
- Insert into current page/canvas and named collections from capture review.
- Flatten-to-rendition export optional; source still immutable by default.

---

### R3 — Browser capture — **not in `feat/clipper`**

**Goal:** Capture browser tabs as first-class sources without desktop-only hacks.

**Scope:**

- `BrowserTab` source via extension / CDP bridge aligned with capture-core IPC.
- Full-page and element captures using tiled LatticeAsset where needed.
- Extension + desktop handshake for permissions and frozen-frame parity where
  applicable.

---

### R4 — Recording foundation — **not in `feat/clipper`**

**Goal:** Screen recording as project bundles, not burned MP4 saves.

**Scope:**

- Native record sample path (still not WebView); timeline recipe model.
- Mediabunny preview and light transcode (tier 4).
- MP4 and other delivery formats as renditions only
  ([ADR 0053](../decisions/0053-lattice-media-asset-model.md)).
- Windows/Linux adapter parity for record contracts (impl may trail macOS).

---

### R5 — Semantic + agentic — **not in `feat/clipper`**

**Goal:** Search, context, and agent tools over captures without conflating inboxes.

**Scope:**

- Index capture metadata and optional vision embeddings ([ADR 0042](../decisions/0042-hybrid-search-qwen3-embedding.md)).
- Agent capabilities for capture-aware tools — separate from Capture Inbox and
  proposal staging ([ADR 0018](../decisions/0018-explicit-capabilities-and-proposed-writes.md)).
- `LatticeRenderer` and `CellSurface` sources for in-app and remote surfaces.

---

### R6 — Media Preview shell maturity — **not in `feat/clipper`**

**Goal:** Unified inspect/preview for stills, tiles, timed media, and recipes.

**Scope:**

- Media Preview shell: tile compositing, timeline scrub, rendition picker.
- Tier 5 export paths (FFmpeg, Cell) behind explicit user actions.
- SVG pipeline (tier 3) for import/export of annotation layers.
- Polish: keyboard review flow, compare source vs recipe, lineage in Inspect.

---

## Dependency sketch

```text
R1 Screenshot foundation (feat/clipper-t1)
    │
    ├── R2 Still markup + insert
    │       │
    │       └── R6 Media Preview shell maturity
    │
    ├── R3 Browser capture
    │
    └── R4 Recording foundation
            │
            └── R5 Semantic + agentic
                    │
                    └── R6 (timed media + preview convergence)
```

R2 and R3 can proceed in parallel after R1 lands; R4 depends on R1 contracts;
R5 depends on inbox files existing; R6 absorbs preview UX from R2 and R4.

---

## Explicit exclusions

The following are **not** part of `feat/clipper-t1` / train R1:

- Still markup, Konva editor, Cropper.js (R2).
- Browser extension capture (R3).
- Recording implementation, Mediabunny, FFmpeg (R4–R6).
- Agent capture tools and semantic indexing (R5).
- Full Media Preview shell (R6).

Implementers should not expand R1 scope to carry these without a new ADR or
train re-cut.
