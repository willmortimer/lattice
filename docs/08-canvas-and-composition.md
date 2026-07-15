# Canvas and Composition

## Product goal

The canvas combines OneNote-like spatial freedom with Notion-like embeds, Airtable-like interfaces, dashboards, drawings, and arbitrary applications. It must remain portable, responsive, and performant.

## Base format

Start with open JSON Canvas as the interoperable spatial skeleton.

JSON Canvas provides:

- Text nodes.
- File nodes.
- Link nodes.
- Groups.
- Edges.
- Coordinates and sizes.

Lattice substantial objects should normally appear as file nodes referencing independent resources.

```json
{
  "nodes": [
    {
      "id": "vision",
      "type": "file",
      "file": "Product/Vision.md",
      "x": 80,
      "y": 80,
      "width": 640,
      "height": 720
    }
  ],
  "edges": []
}
```

## Lattice canvas profile

A sibling sidecar adds richer behavior:

```text
Product Strategy.canvas
Product Strategy.canvas.yaml
```

```yaml
format: lattice-canvas-profile
version: 1
canvas: ./Product Strategy.canvas

reading_order:
  - vision
  - competitor-data
  - market-map

nodes:
  competitor-data:
    renderer: data-view
    resource: ./Research/Competitors.data/views/Overview.view.yaml
    interaction:
      editable: true
      schema_changes: prompt

responsive:
  narrow:
    layout: reading-order
  print:
    layout: paginated
```

## Canvas does not own embedded content

- Editing a page node updates the Markdown page.
- Editing a data node updates the source SQLite resource.
- Rebuilding an artifact updates its package.
- Moving a node updates only composition metadata.

## Hybrid layout model

The canvas supports frames with internal layouts:

- Flow.
- Stack.
- Grid.
- Dashboard.
- Absolute.
- Record detail.
- Spreadsheet.
- Nested canvas.

This avoids making every child an arbitrary pixel-positioned object.

## Responsive and linear representations

Every canvas should have:

- Authored spatial layout.
- Responsive layout for narrow windows and mobile.
- Explicit reading order for accessibility, export, and AI context.
- Optional print/presentation layout.

The user may edit inferred reading order.

## Inline canvas text

Short sticky notes may remain JSON Canvas Markdown text nodes. Long-lived, linked, or substantial text should be convertible to a normal page.

Actions:

```text
Keep as canvas text
Convert to page
```

## Renderer architecture

Use a hybrid compositor:

```text
PixiJS/WebGPU/WebGL scene
├── backgrounds
├── edges
├── groups
├── previews
├── selection geometry
└── thousands of inactive nodes

DOM/native overlay
├── active rich-text editor
├── active data editor
├── forms and controls
├── active notebook
└── PencilKit ink surface
```

At low zoom, resources use thumbnails or simplified summaries. Active resources mount full renderers.

## Performance model

- R-tree or quadtree spatial index.
- Viewport culling.
- Level-of-detail rendering.
- Cached thumbnails.
- Batched edge geometry.
- Imperative camera and pointer state outside React.
- Offscreen artifact and notebook suspension.
- Compiled binary scene cache under `.lattice/`.
- Worker-based edge routing and preview generation.

JSON parsing is not expected to be the dominant bottleneck. A custom binary canonical canvas is not justified for performance alone.

## Possible future custom format

A documented `.lattice-canvas.json` may eventually supersede the sidecar pair if required semantics exceed JSON Canvas cleanly:

- Nested responsive constraints.
- Stable resource bindings.
- Layers and variants.
- Native ink anchors.
- Presentation states.
- Advanced collaboration metadata.
- Plugin-defined node contracts.

The base JSON Canvas export must remain available.

## Canvas state and actions

Canvas-level state may include:

- Selected record.
- Active filters.
- Dashboard parameters.
- Presentation bookmark.
- Current layer.
- User-local viewport.

Canonical shared state and user-local session state must be distinguished.

## Interfaces

Airtable-like interfaces are specialized canvases with:

- Record selectors.
- Record detail panels.
- Editable fields.
- Related records.
- Metrics and charts.
- Forms.
- Buttons and commands.
- Documents and artifacts.
- Conditional visibility.

## Publishing

Canvas publishing modes:

- Static SVG/PNG/PDF snapshot.
- Responsive reading page.
- Interactive web canvas.
- Presentation mode.
- Embedded region in a Lattice App.

## AI-facing representation

MCP/API should offer:

- Raw canvas manifest.
- Linear outline.
- Node inventory.
- Region query.
- Preview image.
- Reading order.

Default agent context should be human-readable rather than raw coordinate JSON.
