# Ink, Apple Pencil, and Spatial Input

## Goal

Lattice should eventually offer OneNote-class handwriting and annotation rather than a mouse-oriented whiteboard approximation.

The iPad implementation should use native PencilKit through a Tauri mobile plugin. Cross-platform ink uses the same open canonical stroke format with platform-specific capture and rendering layers.

## Native iPad architecture

```text
Tauri WebView
├── workspace and canvas UI
├── resource frames
└── coordinate transforms

Native Swift overlay
└── PencilKit canvas
    ├── low-latency ink
    ├── palm rejection
    ├── pressure and tilt
    ├── hover
    ├── squeeze/double-tap
    └── native tool picker
```

A native transparent surface overlays the active Lattice page or canvas region. Coordinates map into document or canvas space.

## Canonical format

Do not use Apple `PKDrawing` as the only representation. Use an open Lattice Ink package:

```text
Lecture Notes.ink/
├── manifest.json
├── strokes.arrow
├── preview.svg
├── recognition.json
└── platform/
    └── pencilkit.cache
```

### Manifest

```json
{
  "format": "lattice-ink",
  "version": 1,
  "id": "019b...",
  "coordinateSystem": {
    "unit": "point",
    "width": 2048,
    "height": 1536
  },
  "layers": [
    {"id": "main", "name": "Notes", "visible": true}
  ]
}
```

### Arrow stroke schema

Each stroke row can contain:

```text
stroke_id
layer_id
tool
color
blend_mode
points: list<struct<
  x, y, time, pressure,
  tilt_x, tilt_y,
  azimuth, altitude,
  width, opacity
>>
transform
```

Arrow provides typed cross-language storage, efficient point arrays, compact null handling, and compatibility with Rust, Swift, JavaScript, and Python.

### Fallbacks

- `preview.svg` for portable viewing.
- Optional PDF or PNG export.
- InkML import/export for pen interoperability.
- Platform-native caches for fast local rendering.

## OneNote-class behaviors

Long-term feature set:

- Low-latency predicted and coalesced stroke input.
- Palm rejection.
- Pencil-only and finger-pan modes.
- Pressure, tilt, azimuth, rotation, and hover.
- Pen, pencil, marker, highlighter, and custom brushes.
- Vector and pixel erasers.
- Lasso selection and transform.
- Shape recognition and correction.
- Handwriting recognition and indexing.
- Convert handwriting to text.
- Search handwritten content.
- Ink replay by timestamp.
- Layers.
- Ruled, grid, music, graph, and custom paper backgrounds.
- Ink anchored to PDF pages, images, diagrams, and canvas frames.
- Audio timestamps linked to strokes for lectures or meetings.
- Squeeze/double-tap tool switching.
- Zoom-independent rendering.
- Collaborative ink and stroke identity.

## Cross-platform input

On Windows, Android, and generic web platforms:

- Pointer Events pressure and tilt.
- Windows Ink integration when available.
- Platform-native stylus APIs through plugins.
- GPU stroke rendering in PixiJS/WebGPU.
- Same canonical Arrow stroke format.

## Ink placement

Ink may be:

- A dedicated page.
- A free layer over a canvas.
- Anchored to a PDF page.
- Embedded in a Markdown page.
- Attached to an image or diagram.
- Used as a canvas annotation layer.

A canvas references the ink package as a normal file resource.

## Recognition and provenance

Recognition output is derived:

```json
{
  "strokeIds": ["..."],
  "text": "deployment architecture",
  "confidence": 0.91,
  "language": "en-US",
  "generatedAt": "..."
}
```

Recognized text improves search but never replaces strokes silently.

## Performance

- Native active-stroke renderer on iPad.
- Append-safe recovery log during capture.
- Chunked Arrow batches.
- GPU tessellation for non-native rendering.
- Simplified previews at low zoom.
- Spatial indexing for lasso and hit testing.
- Background SVG generation.
- Incremental recognition.

## Security and portability

Ink recognition may be local, plugin-provided, or cloud-backed. Cloud recognition requires explicit consent and scoped resource access.
