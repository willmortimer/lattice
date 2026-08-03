# Page (Markdown)

Canonical narrative resource: a Markdown file, optionally with YAML front
matter.

## Example

```text
Notes/Idea.md
```

```markdown
---
title: Idea
---

# Idea

Lattice pages are ordinary Markdown.
```

## Agent notes

- Prefer UTF-8 Markdown on disk.
- Do not wrap pages in a proprietary binary container.
- Wiki-link and embed syntax may appear; unknown directives should degrade
  safely when rendered outside Lattice.
