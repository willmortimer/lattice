# Lattice visual identity

The brand mark is generated, not drawn: a unit cell of a 3D lattice under an
isometric projection, stood on its vertex so the silhouette reads as a
diamond. See [philosophy.md](philosophy.md) for the aesthetic rationale.

- `philosophy.md` — the algorithmic philosophy behind the identity.
- `logo-lab.html` — interactive p5.js explorer for the mark (subdivisions,
  tilt, depth fade, seeded glow nodes, traveling pulses). Open directly in a
  browser; no build step. Download PNG renders from the sidebar.
- `../site/scripts/generate-mark.mjs` — deterministic SVG generator. Run
  `node site/scripts/generate-mark.mjs` to regenerate
  `site/src/assets/lattice-mark.svg` and print the inline variants used in
  `site/src/layouts/Layout.astro` and `apps/desktop/src/App.tsx`.
