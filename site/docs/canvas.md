---
title: Canvas
description: Place resources and notes on an editable JSON Canvas, connect them, and keep an accessible outline.
---

Canvas is the spatial view of a workspace. The canonical file uses JSON Canvas;
referenced pages, files, tables, and views remain independent resources.

## Build a canvas

1. Create or open a `.canvas` resource.
2. Choose **Place resource** and filter the workspace list.
3. Select a resource to place it at the current canvas position.
4. Choose **Add note** for a canvas-owned sticky note.
5. Drag nodes to move them and use the southeast handle to resize.
6. Choose **Connect**, click the first node, then the second to draw an arrow.

Select a node and press an arrow key for precise movement; hold Shift for a
larger step. Delete or Backspace removes the current node or edge. **Fit** brings
the complete scene back into view.

## Open and inspect resources

Activate a file node to open its source resource. Data nodes may target a saved
view inside a `.data` package, so a canvas can point directly to a board,
calendar, or interface without copying the records.

Choose **Outline** for a semantic DOM list of nodes. The outline provides a
keyboard and screen-reader-friendly reading order alongside the GPU scene.

## External edits

Other JSON Canvas tools may edit the same file. Lattice watches the materialized
canvas and reports invalid or conflicting changes rather than inventing a
precise semantic operation it did not observe.
