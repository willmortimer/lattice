# Documentation Sites and Publishing

## First-class docs projects

Lattice should support selecting a folder and turning it into a documentation project without inventing a proprietary documentation language.

```text
docs/
├── docs.lattice.yaml
├── index.md
├── guides/
├── reference/
├── assets/
└── generated/
```

## Project manifest

```yaml
format: lattice-docs-project
version: 1
title: Lattice Documentation
content:
  root: .
  home: index.md
navigation:
  mode: filesystem
renderer:
  preset: starlight
features:
  search: true
  dark_mode: true
  edit_links: true
  code_copy: true
  mermaid: true
sources:
  - type: markdown
    path: .
  - type: openapi
    path: ../api/openapi.yaml
    mount: /api
output:
  directory: ../dist/docs
```

## Folder-to-docs UX

1. Select folder.
2. Choose **Create documentation site**.
3. Lattice inspects structure.
4. Infer README/index home, guides, reference, ADRs, notebooks, OpenAPI, examples, and assets.
5. Preview proposed navigation.
6. Select renderer preset.
7. Validate and preview.
8. Export or publish.

## Renderer adapters

### Astro Starlight

Recommended default for general product and technical documentation.

### VitePress

Fast Markdown-first technical docs and Vue-oriented customization.

### Docusaurus

React/MDX-heavy, versioned documentation and blog use cases.

### mdBook

Linear books, Rust projects, tutorials, and manuals.

### MkDocs

Simple Python-centric Markdown docs and existing repositories.

### Quarto

Jupyter-heavy, scientific, book, report, and multi-format publishing.

### Custom Lattice App

Maximum flexibility using the app SDK and UI kit.

Lattice writes normal Markdown and adapter configuration. It does not require the selected generator to become the canonical page format.

## Generated reference sources

Official adapters should support:

- OpenAPI: Redoc, Swagger UI, or native Lattice API reference.
- AsyncAPI: event-driven APIs.
- TypeScript: TypeDoc JSON or HTML.
- Rust: rustdoc and doctests.
- Python: Sphinx or pdoc plugins.
- Kotlin: Dokka.
- Java: Javadoc.
- Go: pkgsite/godoc.
- C/C++: Doxygen.
- GraphQL schema.
- Protobuf descriptors.
- JSON Schema.
- Terraform providers/modules.
- Helm values and schemas.
- CLI help/manpage schemas.
- Database schema and ER documentation.

## Lattice opinionated value

Lattice adds quality and composition rather than a proprietary syntax:

- Navigation generation and explicit ordering.
- Broken-link validation.
- Missing image and duplicate-anchor checks.
- Orphan detection.
- Front-matter validation.
- Code-snippet reference validation.
- Executable example tests.
- OpenAPI/AsyncAPI/schema validation.
- Heading and accessibility checks.
- Stale generated-reference detection.
- Asset optimization warnings.
- Search-index generation.

## Code documentation

Support file-backed snippets, semantic symbol extraction, tested examples, code groups, diff annotations, and notebook-derived examples.

## Rich resources in docs

Publish:

- Markdown pages.
- Mermaid and Graphviz.
- Vega-Lite charts.
- Jupyter outputs.
- Data tables and snapshots.
- Canvas images or interactive canvases.
- API references.
- Artifacts and Lattice Apps.
- PDF and media embeds.

Each resource chooses static snapshot or interactive publishing.

## Documentation outputs

- Static site.
- PDF.
- EPUB.
- DOCX/ODT.
- Typst/LaTeX.
- Search index.
- `sitemap.xml`.
- `llms.txt` and optionally `llms-full.txt`.
- Human-readable context export.
- Offline archive.

## Publishing providers

- Local directory.
- GitHub Pages.
- Cloudflare Pages.
- S3-compatible hosting.
- Lattice server.
- Custom provider plugin.

Publishing is a provider interface, not hard-coded hosting.

## Versioning and localization

Long-term:

- Versioned documentation trees.
- Branch/tag sources.
- Language variants.
- Shared translated resource identity.
- Version-aware links.
- Diff between releases.

## Security

Executable notebooks, artifacts, and generated code do not run during publishing without an approved task environment and capabilities.
