---
title: Inspect
description: Details for the thing you already have open.
---

# Inspect

Open **Inspect** when you need more than the main editor shows — links,
history, schema, cloud backup, and diagnostics for **this** page, table, or
file.

## Open Inspect

1. Select a file in the **Files** tree (or open it in the main area).
2. Click **Show inspector** in the header, or right-click the file and choose
   **Inspect**.

Close Inspect with **Hide inspector** or the close button. It is detail for the
selected item, not a separate app mode.

## Sections

Use the tabs across the top of Inspect:

| Section | What you get |
| --- | --- |
| **properties** | Kind, path, format, resource ID, **Authority**, **Editing authority**, **Materialization** |
| **links** | Backlinks (pages only) |
| **graph** | How this item connects to neighbors |
| **history** | Recent command history for this resource |
| **schema** | Table / data-app schema when relevant |
| **source** | Raw source view |
| **permissions** | Capability and access hints |
| **diagnostics** | Errors and reconciliation status |

**Authority** tells you who owns the canonical copy (for example **Local** or
**Cloud**). **Editing authority** shows how the page is being edited:
**Plain file** or **Collaborative**. That is separate from **Authority** — it
does not mean the file is backed up to the cloud. **Materialization** tells you
how much of the file is present on disk (for example **Cached** or **Metadata
only**).

## Back up to Lattice Cloud

For most files (not folders), Inspect shows cloud actions under **properties**:

1. Sign in under **Settings → Cloud account**.
2. Select the resource in the tree.
3. Open **Inspect → properties**.
4. Click **Back up to Lattice Cloud**.

After a successful upload, **Authority** becomes **Cloud**. You can also back
up from the **Files** tree context menu (**Back up to Lattice Cloud**) or from
the command palette when a resource is selected.

**Reopen from cloud** fetches the cloud copy and updates the workspace file when
the content is text. The button is available when **Authority** is already
**Cloud**.

If backup fails because the cloud already has a different version of this file,
you may see:

> Local content changed since this resource was bound in cloud. MVP keeps a
> single hash per resource and cannot overwrite that binding. Check Inspect →
> Properties for authority and content hash.

Check **Authority** and **Content hash** on the properties tab to compare local
and cloud state.

## Sync conflict (Keep local / Take cloud)

When local and cloud versions already disagree, Inspect **properties** shows
**Keep local** and **Take cloud**. Nothing is overwritten until you choose:

- **Keep local** — keep the file on this machine and update the cloud binding
  to match.
- **Take cloud** — replace the local file with the cloud copy.

While you are signed in with a workspace open, Lattice checks the cloud in the
background about every 30 seconds. **Keep local** / **Take cloud** still appear
only when there is a conflict — nothing is overwritten until you choose.

## Encrypted workspace backup

This is a snapshot of the **whole workspace**, not one file. It is not the same
as **Back up to Lattice Cloud**. You need an open workspace to **back up**.

**Back up** (workspace already open):

1. Sign in under **Settings → Cloud account**.
2. Under **Encrypted workspace backup**, click **Back up workspace**.

**Restore on a new Mac** (empty Lattice, no workspace yet):

1. Click **Sign in to Lattice Cloud**, then **Restore encrypted backup**.
2. Pick a cloud workspace, a backup, and a destination folder.
3. Click **Restore backup**. Lattice opens that folder as the workspace.
   If the browser shows **Opening Lattice…**, click **Open Lattice**.

You do not need to create a Personal workspace first.

**Restore when a workspace is already open:**

- **Settings → Cloud account → Encrypted workspace backup** — pick a backup
  and a destination folder, then **Restore backup**.
- **Inspect → properties** can restore a picked backup into the **open**
  workspace.

Existing files with different content are skipped.

## File conflicts vs cloud

If you edited a page in Lattice and the same file changed on disk outside the
app, the page editor may offer **Keep local**, **Keep incoming**, or **Keep
both**. That is a **local file conflict**, not a cloud backup or sync-conflict
action. Cloud backup uses **Back up to Lattice Cloud** and **Reopen from
cloud**. Cloud sync disagreement uses **Keep local** / **Take cloud** in
Inspect.
