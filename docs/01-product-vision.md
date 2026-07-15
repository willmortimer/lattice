# Product Vision

## The problem

Current products split the information-work space into artificial categories:

- Cloud-first document suites provide polish but are slow, server-dependent, difficult to automate freely, and weakly portable.
- Local Markdown tools preserve ownership but require layers of plugins and conventions to approximate rich documents, databases, canvases, dashboards, and applications.
- Database builders provide excellent operational workflows but trap records, interfaces, and automations in hosted proprietary bases.
- AI second-brain products add capture, semantic retrieval, typed notes, or synthesized context, but frequently introduce opinionated ontologies, opaque indexes, agent-centric organization, and proprietary memory systems.
- Notebook and BI tools are excellent at computation or presentation but are disconnected from normal documents, project structure, and daily workspace behavior.

The unmet need is a fast local workspace that treats narrative documents, relational applications, analytical data, computation, visual composition, and automation as equally legitimate resources.

## Product thesis

Lattice is the open information studio:

> Obsidian's files, OneNote's canvas and ink, Notion's interaction quality, Airtable's relational application model, Jupyter's computation, and modern BI's presentation layers—composed through open formats and one semantic runtime.

Lattice is not merely a note application with more blocks. It is also not an operating system in the marketing sense. It is a coherent desktop and eventually mobile environment for compound information.

## Primary product objects

Lattice centers several distinct resource types instead of forcing everything into a universal block or object model:

- **Page:** narrative Markdown content.
- **Canvas:** spatial or structured composition referencing other resources.
- **Data application:** mutable typed relational data, views, forms, interfaces, and actions.
- **Analytical dataset:** large or append-oriented data, commonly Parquet, queried through DuckDB.
- **Notebook:** Jupyter-compatible interactive computation.
- **Ink resource:** open stroke data with native platform capture and portable previews.
- **Artifact:** sandboxed HTML/CSS/JavaScript mini-application.
- **Lattice App:** complete source-backed web application or published experience.
- **View:** a presentation or query over another resource.
- **Workflow/task:** inspectable automation and execution resources.
- **File:** any ordinary content not requiring a special native model.

## Human-readable first

Canonical organization should make sense in Finder, Explorer, Git, a terminal, VS Code, or any capable text editor.

Human-readable first does not mean everything is plain text. SQLite, Parquet, Arrow, PDF, images, and notebooks are legitimate open formats. It means:

- Names and paths communicate purpose.
- Every package has a readable manifest and README where appropriate.
- Rich resources have fallbacks or previews.
- Generated resources identify inputs and builders.
- AI-specific indexes never replace source content.
- Token efficiency is handled by retrieval APIs rather than by compressing canonical information into opaque structures.

## AI-native without an embedded brain

Lattice learns from post-AI knowledge products in limited, useful ways:

- Low-friction capture and inboxes.
- Stable page and block references.
- Typed notes and optional schemas.
- Related-content discovery.
- Collections that do not require duplication.
- Context bundles with citations and provenance.
- Semantic search alongside deterministic search.
- Easy conversion of unstructured material into structured data or applications.

Lattice does not require:

- A proprietary agent.
- Hidden autonomous reorganization.
- A mandatory personal ontology.
- A canonical AI memory graph.
- Model-generated text mixed invisibly with authored source material.
- A paid plan for CLI, API, MCP, plugins, or local automation.

External agents interact with Lattice through raw files, semantic commands, CLI, local API, MCP, and approved workflows.

## Target users

- Engineers combining specifications, repositories, datasets, logs, architecture diagrams, runbooks, notebooks, and dashboards.
- Researchers combining sources, citations, PDFs, experimental data, notebooks, figures, and written synthesis.
- Founders and small teams combining product documents, CRM-like data, hiring, customer research, analytics, and published experiences.
- Analysts combining local and remote databases, semantic models, notebooks, dashboards, and narrative reports.
- Students using typed notes, handwriting, PDFs, code, data, diagrams, and course notebooks.
- Writers and designers building research-heavy projects with canvases and artifacts.
- People who want OneNote-like immediacy without proprietary storage.

## Competitive differentiation

Lattice aims to occupy a missing intersection:

```text
Rich narrative documents
+ serious structured and analytical data
+ spatial canvas and native ink
+ arbitrary interactive applications
+ local-first open files
+ unrestricted automation and AI access
```

The moat is the combination of:

1. A credible multi-format workspace specification.
2. A performant local resource runtime.
3. A safe semantic command and transaction model.
4. SQLite and Airtable-like application UX.
5. Parquet, DuckDB, Arrow, Jupyter, and remote database integration.
6. Hybrid DOM/GPU/native rendering.
7. Secure plugins, artifacts, scripts, and full applications.
8. Optional self-hosted collaboration that does not become canonical storage.

## Product posture

Lattice should feel simple at first contact:

```text
New page
New canvas
Open file
Quick capture
```

Its deeper systems should emerge only when relevant. The product supports almost anything, but shows only what the current context and enabled capabilities require.

## Positioning language

Preferred concise definition:

> Lattice is a fast local-first workspace for documents, data applications, notebooks, canvases, drawings, dashboards, and software—built from open resources and programmable through a shared CLI, API, and MCP model.

Preferred principle:

> Unlimited composition, not unlimited ambient complexity.
