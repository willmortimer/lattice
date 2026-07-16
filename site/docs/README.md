# Public documentation source

This folder is the curated source for the Starlight site. It is intentionally
smaller than the repository-level `docs/` architecture corpus.

- Update these pages for user-facing product behavior and onboarding.
- Keep deep architecture, future-phase specifications, and ADRs in `/docs`.
- Link to the canonical repository document when a public page summarizes it.
- `site/scripts/sync-docs.mjs` copies this folder into generated Starlight
  content and builds the sidebar from `navigation.json`.
