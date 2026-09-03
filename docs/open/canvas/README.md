# Canvas (JSON Canvas)

Canonical spatial resource: [JSON Canvas](https://jsoncanvas.org/) plus an
optional Lattice profile sidecar for reading order, bindings, and layout hints.

## Example

```text
Boards/Map.canvas
Boards/Map.canvas.lattice.yaml   # optional profile sidecar (when present)
```

## Agent notes

- `.canvas` files should remain valid JSON Canvas for other tools.
- Lattice-specific behavior belongs in documented profile fields, not in
  breaking the base canvas schema.
