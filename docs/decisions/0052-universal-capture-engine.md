# ADR 0052: Universal capture engine with native hot paths

## Status

Accepted (implementation on `feat/clipper-t1`, train R1).

## Context

Quick capture is part of the first hero workflow
([design review addendum §8.2](../38-design-review-addendum.md#82-move-capture-earlier)),
but the open client has no ADR that locks capture contracts. Prose and prototypes
drift toward WebView overlays, `/usr/sbin/screencapture`, and conflating capture
inbox with agent proposal staging. Voice dictation already proved that trusted
native capture must not live in the Tauri WebView hot path
([voice ADR 0008](../voice/adr/0008-native-client-capture.md)).

Lattice needs one capture subsystem that can grow from macOS still screenshots
to window/region selection, browser tabs, in-app renderer surfaces, Cell
surfaces, and later screen recording — without rewriting destinations, inbox
semantics, or UI exclusion each time.

## Decision

### Subsystem shape

- **`lattice-capture-core`** owns capture session lifecycle, source/destination
  contracts, frozen-frame selection state, and IPC toward the desktop shell.
- **Platform adapters** implement pixel acquisition and OS integration:
  - **macOS** first (`feat/clipper-t1` / R1).
  - **Windows, Linux, browser extension, Lattice renderer, and Cell surface**
    adapters follow in later trains; they plug the same core contracts rather
    than forking per-surface capture code.

### Hot path is native

Selection overlays, frozen-frame presentation, and sample acquisition run in
**native code (Rust and/or Swift)** owned by the Tauri shell or a small native
helper. The **Tauri WebView is not in the selection or record sample path** —
same principle as voice capture
([voice ADR 0008](../voice/adr/0008-native-client-capture.md)). React coordinates
session intent, destination choice, and post-capture review; it does not own
frame buffers or compositor timing.

### Capture sources (v1 contract)

The core enumerates sources without requiring every adapter to implement every
kind on day one:

| Source | Meaning |
| --- | --- |
| `Display` | Full display backing store |
| `Window` | Single composited window |
| `Region` | User-selected rectangle (against frozen frame) |
| `BrowserTab` | Browser extension / CDP surface (later train) |
| `LatticeRenderer` | In-app Pixi/canvas/renderer export surface |
| `CellSurface` | Remote or sandboxed Cell display surface |

R1 implements macOS still capture for display, window, and region.

### Destinations

Each successful capture may fan out to one or more destinations:

- **Clipboard** (platform image pasteboard).
- **Capture Inbox** — workspace-local staging folder for unsorted captures
  (see below).
- **Current note or canvas** — insert at cursor or spatial anchor when a focused
  editable surface is active.
- **Named collection** — user- or template-defined folder (e.g. `Screenshots/`).

**Default destination:** **clipboard + Capture Inbox** on every still capture.
Other destinations are explicit opt-in per session or preference.

### Capture Inbox ≠ proposal inbox

**Capture Inbox** is a **workspace capture folder** (profile `quickNoteDirectory`
or template-defined capture path). It holds immutable capture files awaiting
triage into notes, canvases, or collections.

It is **not** the agent **proposal inbox**, draft overlay, or semantic-command
staging surface ([ADR 0018](0018-explicit-capabilities-and-proposed-writes.md)).
Agent-generated captures and tool outputs use separate capabilities and paths;
this ADR only notes that agent capture is a distinct concern (see R5).

### Exclude Lattice UI from pixel capture

Captured pixels must not include Lattice chrome, overlays, or selection UI.
macOS uses ScreenCaptureKit content filters and shareable-content exclusion
(`SCShareableContent`, `SCContentFilter`). Windows will use
`WDA_EXCLUDEFROMCAPTURE` (and equivalent) in a later adapter. Exclusion is
adapter-owned but contract-required: adapters must not ship without a verified
self-exclusion path for the Lattice process.

### macOS still capture API

macOS stills use **ScreenCaptureKit**, not `/usr/sbin/screencapture`:

- `SCShareableContent` for display/window inventory.
- `SCContentFilter` for target selection and Lattice self-exclusion.
- `SCScreenshotManager` (or equivalent SCK still pipeline) for framebuffer
  acquisition.

`screencapture` is unsuitable for multi-monitor frozen overlays, per-window
filtering, and consistent exclusion semantics.

### Frozen multi-monitor overlay

Region and window selection present a **frozen snapshot** across all attached
displays. The user selects against that buffer (not live compositor frames) so
motion, cursor flicker, and partial monitor updates do not corrupt selection.
Live preview resumes only after commit or cancel.

### Recording (contract only in R1)

Screen **recording** is part of the universal engine contract but **not** R1
implementation scope. The API returns a **project bundle** (timeline, source
refs, edit metadata) — not a burned-in MP4 export. MP4 and other delivery
renditions are produced by the media asset pipeline
([ADR 0053](0053-lattice-media-asset-model.md), train R4). R1 may stub recording
entry points without implementing capture.

### Authority

Capture files written to the workspace flow through the same semantic command
and resource model as other File resources
([ADR 0007](0007-semantic-command-transaction-core.md),
[ADR 0035](0035-format-first-file-resources-and-resource-format-profile.md)).
Clipboard writes are OS-side effects outside canonical workspace state.

## Consequences

- New crates `lattice-capture-core` and `lattice-capture-macos` (or equivalent)
  land behind the desktop shell; no capture hot path in the WebView.
- Global shortcut and menu-bar entry points invoke native capture sessions, not
  DOM overlays.
- Capture Inbox paths are profile-visible and template-seeded; documentation and
  settings must not label them as agent proposal queues.
- Browser, renderer, Cell, and recording adapters can ship incrementally without
  changing destination or inbox contracts.
- Follow-on ADR [0053](0053-lattice-media-asset-model.md) and
  [capture roadmap](../architecture/capture-and-media.md) sequence markup,
  browser capture, recording, and preview maturity across trains R2–R6.
