---
title: Collaborative pages and boards
description: Shared editing on pages and canvases with a resource ID.
---

# Collaborative pages and boards

By default, pages and canvases save as ordinary workspace files (**Plain file**).
Items that have a registry **Resource ID** can switch to **Collaborative** mode
for live co-editing.

## Plain file vs Collaborative

Open a page or canvas that has a resource ID. In the toolbar you will see:

| Mode | Behavior |
| --- | --- |
| **Plain file** | Pages save markdown; canvases apply JSON Canvas patches to the `.canvas` file |
| **Collaborative** | Live shared editing through the Yrs journal (not per-keystroke file writes) |

Switch with the **Plain file** / **Collaborative** toggles in the page chrome or
the canvas toolbar.

Your choice is remembered when you reopen the item in the same workspace, as
long as it still has a resource ID. Switching back to **Plain file** is
remembered the same way.

In **Collaborative** mode on pages, a **Comments** button may appear for inline
discussion.

**Inspect → properties** shows **Editing authority** with the same labels
(**Plain file** or **Collaborative**). That is separate from **Authority**
(which shows who owns the canonical copy, such as **Local** or **Cloud**).

## When Collaborative is not offered

The toggles only appear when the item already has a resource ID in Inspect →
**properties**. New or unregistered files stay on **Plain file** until the
registry assigns an ID.

## Cloud catch-up

When you are signed in to Cloud, live collaborative edits can catch up through
the cloud. Local editing still works if you are offline. This is **not**
**Inspect → Back up to Lattice Cloud** (see [Inspect](inspect.md)).
