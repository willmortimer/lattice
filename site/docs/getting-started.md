---
title: Getting started
description: Create a workspace, learn the desktop shell, and complete the first useful Lattice workflow.
---

## Create or open a workspace

On first launch, choose a workspace template:

- **Personal** for inbox, projects, areas, library, and journal.
- **Project** for one outcome with mixed working resources.
- **Research** for questions, sources, notes, experiments, and outputs.
- **Data Lab** for sources, datasets, queries, notebooks, and reports.
- **Blank** for only the readable workspace manifest.

Choose a title and location. Lattice writes a `lattice.yaml` file into that
directory; it does not upload or relocate the folder.

Use **Open workspace…** when the directory already contains a Lattice manifest.
The First Look sample workspace is useful when you want populated pages,
canvases, data apps, datasets, notebooks, tasks, workflows, and artifacts to
explore safely.

## Learn the shell

- **Activity rail:** files, search, proposals, settings, and optional terminal.
- **Resource tree:** folders and resources in the real workspace directory.
- **Tabs:** open resources; back and forward preserve navigation context.
- **Main surface:** the editor or renderer for the selected resource.
- **Inspect:** links, history, schema, source, permissions, and diagnostics.

The tree supports rename, drag-to-move, multi-select with ⌘/Ctrl-click, range
selection with Shift-click, and OS Trash for deletion.

## Your first ten minutes

1. Press **⌘/Ctrl+P**, choose **New page**, and give it a title.
2. Type `/` for block commands or `[[` to link another page.
3. Press **⌘/Ctrl+N** and save a Quick Note into the workspace inbox.
4. Press **⌘/Ctrl+K** and search for text from either note.
5. Open **Inspect → Links** to see backlinks.
6. Create a Table from the command palette or import a CSV.
7. Create a Canvas, choose **Place resource**, and place the page and table.
8. Open the workspace directory externally and confirm the Markdown, Canvas,
   and `.data` package are ordinary resources on disk.

## Saving and external edits

Page edits autosave through the command core. **⌘/Ctrl+S** saves immediately.
If another program changes an open file, Lattice reloads it when safe or shows a
conflict instead of overwriting either version.

Next: [Pages and capture](/docs/pages-and-capture/) or
[Tables and data apps](/docs/tables-and-data-apps/).
