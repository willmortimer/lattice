---
title: Collaborative pages
description: Opt in to shared editing on pages with a resource ID.
---

# Collaborative pages

By default, pages save as a normal markdown file on disk (**Plain file**). Labs
adds an optional **Collaborative** mode for live co-editing when a page has a
registry **Resource ID**.

## Turn on the Labs toggle

1. Open **Settings** (gear on the activity rail).
2. Go to **Features**.
3. Scroll to **Labs**.
4. Enable **Enable collaborative page editor (Labs)**.

The setting is titled **Labs collaborative page editor**. It is off until you
opt in.

## Plain file vs Collaborative

Open a page that has a resource ID. In the page toolbar you will see:

| Mode | Behavior |
| --- | --- |
| **Plain file** | Standard markdown editing; saves to the file on disk |
| **Collaborative** | Live shared editing (not the same as autosaved markdown) |

Switch with the **Plain file** / **Collaborative** toggles above the editor.

Your choice is remembered when you reopen the page in the same workspace, as long
as Labs is still on and the page still has a resource ID. Switching back to
**Plain file** is remembered the same way.

In **Collaborative** mode, a **Comments** button may appear for inline
discussion.

**Inspect → properties** shows **Editing authority** with the same labels
(**Plain file** or **Collaborative**). That is separate from **Authority**
(which shows who owns the canonical copy, such as **Local** or **Cloud**).

## When Collaborative is not offered

The toggles only appear when Labs is enabled **and** the page already has a
resource ID in Inspect → **properties**. New or unregistered pages stay on
**Plain file** until the registry assigns an ID.

## Related Labs settings

Under the same **Labs** section you may see **Labs remote Yrs provider** with
**Enable remote Yrs provider (Labs)**. That toggle stays disabled until
**Enable collaborative page editor (Labs)** is on.

When you are signed in to Cloud and the remote Yrs Labs toggle is on, live
collaborative edits can catch up through the cloud. Local editing still works
if you are offline. This is **not** **Inspect → Back up to Lattice Cloud** (see
[Inspect](inspect.md)).
