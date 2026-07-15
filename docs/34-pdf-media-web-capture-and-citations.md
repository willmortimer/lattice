# PDF, Media, Web Capture, and Citations

## Purpose

Lattice is not limited to text and tables. Research, design, education, operations, and technical work frequently depend on PDFs, images, audio, video, web pages, highlights, and citations. These should remain ordinary files with open annotations and provenance rather than being ingested into an opaque knowledge store.

## PDF support

PDFs remain canonical `.pdf` files. Lattice adds optional sidecar resources for:

- highlights;
- comments;
- freehand ink;
- shapes;
- page links;
- extracted text caches;
- citation anchors;
- thumbnails;
- OCR results;
- reading position.

Example:

```text
Papers/
├── local-first-software.pdf
├── local-first-software.pdf.annotations.json
├── local-first-software.pdf.ink/
└── .lattice-cache/
    ├── text/
    └── thumbnails/
```

Annotations should use a documented schema and preserve:

- page number;
- PDF coordinates;
- selected text where applicable;
- annotation type;
- author;
- timestamp;
- color and presentation;
- stable annotation identity;
- optional link to a page, record, or citation.

Where possible, Lattice should import and export standard PDF annotations. Application-specific metadata belongs in a documented sidecar instead of rewriting the source PDF unnecessarily.

## OCR and extracted text

Extracted text and OCR are derived resources:

- never silently replace the source;
- retain page and bounding-box mappings;
- record the extractor and version;
- indicate confidence;
- remain rebuildable;
- support search and citation without becoming the sole canonical copy.

OCR and media analysis should run through plugins or tasks so users can choose local, remote, or domain-specific engines.

## Image and spatial annotation

Images remain ordinary PNG, JPEG, WebP, AVIF, SVG, TIFF, or domain-specific files. Lattice can attach:

- regions;
- labels;
- measurements;
- ink;
- arrows and shapes;
- linked records;
- captions;
- segmentation masks;
- provenance.

The annotation representation should be open JSON or another documented package. Large scientific images may use tiled formats or specialized plugins.

## Audio and video

Lattice should support:

- playback;
- waveform and timeline views;
- timestamped notes;
- transcript import;
- captions and subtitles;
- linked clips;
- chapter markers;
- canvas embedding;
- external-editor round-tripping.

Canonical media remains in established formats. Transcripts are ordinary text, Markdown, WebVTT, SRT, or JSON resources with timestamp mappings.

A useful pattern is:

```text
Interview/
├── recording.m4a
├── transcript.md
├── transcript.vtt
├── notes.md
└── interview.yaml
```

## Web capture

A first-party web-capture capability should support several capture modes:

- bookmark with title, URL, description, and retrieval date;
- readable article snapshot;
- full-page HTML archive;
- PDF snapshot;
- selected text and highlights;
- screenshot;
- structured metadata;
- link-only reference;
- live external embed.

Every capture should preserve:

- original URL;
- retrieval time;
- content hash;
- capture mode;
- page title and author where available;
- canonical URL;
- permissions and privacy status;
- whether the content is mirrored or only linked.

Captured content should be stored in ordinary directories:

```text
Sources/
└── Example Article.capture/
    ├── README.md
    ├── capture.yaml
    ├── article.md
    ├── original.html
    ├── screenshot.webp
    └── assets/
```

The browser extension, share sheet, CLI, and API should all invoke the same import command.

## Highlights and source anchors

Highlights should refer back to source locations:

```yaml
id: 019b...
source: ./Example Article.capture/capture.yaml
selector:
  type: text-quote
  exact: "Local-first software..."
  prefix: "..."
  suffix: "..."
captured_at: 2026-07-14T20:00:00-07:00
```

For PDFs, selectors use page and coordinates. For audio/video, they use timestamps. For notebooks and code, they use cell, symbol, or line anchors.

## Citations

Lattice should interoperate with established citation formats:

- CSL JSON;
- BibTeX/BibLaTeX;
- RIS;
- DOI;
- Zotero libraries and Better BibTeX exports;
- Crossref and other metadata providers through plugins.

A citation library may be represented as:

- a SQLite data application for rich editable records;
- a CSL JSON or BibTeX file for interchange;
- linked PDFs and notes;
- generated bibliographies through Pandoc or CSL processors.

Lattice should support:

- citekeys;
- citation insertion in Markdown;
- citation autocomplete;
- bibliography generation;
- citation style selection;
- source-to-note links;
- missing-metadata warnings;
- duplicate detection;
- attachment management;
- PDF highlight links;
- citation export.

## Research workflow

A first-party research capability pack may provide:

- source inbox;
- literature database;
- PDF reader and annotation;
- citation library;
- evidence matrix;
- reading status;
- related pages;
- notebook integration;
- generated reports;
- context-bundle export.

This remains an optional pack built from ordinary resources, not a mandatory personal knowledge ontology.

## Privacy and security

Web capture and media processing must:

- avoid uploading private material without explicit approval;
- make network calls visible;
- isolate untrusted HTML;
- sanitize active content;
- store credentials through secret providers;
- distinguish local extraction from remote services;
- preserve copyright and access metadata where appropriate.
