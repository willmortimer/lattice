---
title: Collaborative pages
description: Shared editing on pages with a resource ID.
---

# Collaborative pages

By default, pages save as a normal markdown file on disk (**Plain file**). Pages
that have a registry **Resource ID** can switch to **Collaborative** mode for
live co-editing.

## Plain file vs Collaborative

Open a page that has a resource ID. In the page toolbar you will see:

| Mode | Behavior |
| --- | --- |
| **Plain file** | Standard markdown editing; saves to the file on disk |
| **Collaborative** | Live shared editing (not the same as autosaved markdown) |

Switch with the **Plain file** / **Collaborative** toggles above the editor.

Your choice is remembered when you reopen the page in the same workspace, as
long as the page still has a resource ID. Switching back to **Plain file** is
remembered the same way.

In **Collaborative** mode, a **Comments** button may appear for inline
discussion.

**Inspect → properties** shows **Editing authority** with the same labels
(**Plain file** or **Collaborative**). That is separate from **Authority**
(which shows who owns the canonical copy, such as **Local** or **Cloud**).

## When Collaborative is not offered

The toggles only appear when the page already has a resource ID in Inspect →
**properties**. New or unregistered pages stay on **Plain file** until the
registry assigns an ID.

## Cloud catch-up

When you are signed in to Cloud, live collaborative edits can catch up through
the cloud. Local editing still works if you are offline. This is **not**
**Inspect → Back up to Lattice Cloud** (see [Inspect](inspect.md)).
