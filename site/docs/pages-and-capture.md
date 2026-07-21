---
title: Pages and capture
description: Write Markdown pages, create links, embed resources, and use Quick Note or local dictation.
---

Pages are Markdown files with an optional frontmatter header. The rich editor,
source view, and preview all operate on that same file.

## Create and edit a page

1. Press **⌘/Ctrl+P** and choose **New page**.
2. Enter a title and choose its folder.
3. Type normally; changes autosave after a short debounce.
4. Press **⌘/Ctrl+S** when you want to save immediately.

Type `/` at the start of a block to open contextual commands for headings,
lists, callouts, code, images, and embeds. Paste Markdown or sanitized HTML to
convert it into the page rather than storing opaque clipboard markup.

## Link pages

Type `[[` and begin a page title. Choose a result and continue typing. The link
stays readable in Markdown and becomes navigable in Lattice.

Open **Inspect → Links** to see pages that link to the current page. Search and
the command palette can also jump to a page by title or path.

## Embed another resource

Resource embeds use a readable directive:

```markdown
:::lattice-embed
resource: CRM.data/views/Board.yaml
fallback: "Open the CRM board"
:::
```

Images and common files receive previews. Pages show excerpts. Data resources,
tasks, and interfaces show structured cards. Sandboxed artifacts can opt into
an interactive inline mode:

```markdown
:::lattice-embed
resource: Artifacts/ContactPulse.artifact
mode: interactive
height: 320
:::
```

## Use Quick Note

Press **⌘/Ctrl+N** from the main window or use the system Quick Note shortcut.
Type a note and save it to the configured inbox. Escape cancels without leaving
an empty file.

On supported macOS builds, hold the microphone control while speaking and
release to finalize. Provisional text is visual only; Lattice commits the final
transcript once, after local normalization. Voice setup and model status live
in Settings and remain opt-in.

## Resolve a conflict

If the materialized page changed outside Lattice while you were editing, the
editor shows a conflict banner. Reload to accept the external version, keep or
copy your draft, or open the revision information in Inspect before deciding.
