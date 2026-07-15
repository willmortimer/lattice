# ADR 0006: React coordinates the shell; specialized surfaces own hot paths

## Status
Accepted

## Context
The application requires a rich editor, GPU canvas, large grids, charts, notebooks, PDFs, code editors, maps, and native ink. A single reactive component tree would create performance and lifecycle problems.

## Decision
Use React 19 and Vite for the shell and broad ecosystem compatibility. ProseMirror/Tiptap owns document editing, PixiJS or an equivalent scene engine owns canvas movement, dedicated grids own scrolling, Jupyter owns kernel state, chart libraries own rendering, and native PencilKit owns active iPad ink.

## Consequences
- React ecosystem risk is low without making React the rendering engine for everything.
- Integration contracts and explicit lifecycle management are required.
- Quick note uses a separate minimal frontend entry point.
