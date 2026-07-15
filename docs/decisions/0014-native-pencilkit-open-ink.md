# ADR 0014: Use native PencilKit with an open Lattice Ink package

## Status
Accepted

## Context
Web-only pointer drawing is unlikely to match OneNote or native Apple Notes latency, palm rejection, pressure, tilt, hover, squeeze, and tool behavior on iPad. Apple-specific serialized drawings are not a sufficient portable canonical format.

## Decision
Use a native Swift/PencilKit overlay for active iPad ink through a Tauri mobile plugin. Store canonical ink in a documented package using JSON metadata, Arrow stroke data, and SVG preview, with optional platform caches and InkML interchange.

## Consequences
- iPad drawing can reach native quality.
- Cross-platform renderers share one open stroke model.
- Conversion between native and canonical stroke representations must preserve fidelity.
