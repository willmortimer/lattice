---
title: Tables and data apps
description: Create typed SQLite tables, edit records, save views, add relations and formulas, and build forms or interfaces.
---

A Lattice Table is a SQLite-backed `.data` package from the moment you create
it. You can begin with a simple grid and add application behavior gradually.

## Create or import a table

- Press **⌘/Ctrl+P** and choose **New table…** to define a package and columns.
- Choose **Import table…** to review a CSV or other supported tabular file,
  confirm inferred field types, and create a package.

Open the `.data` package from the resource tree. Use **Add row** to create a
record and edit cells directly in Grid view.

## Choose a layout

Use the view picker for:

- **Grid:** dense spreadsheet-like editing.
- **List:** compact readable records.
- **Board:** cards grouped by a field.
- **Gallery:** cards with a selected cover or label field.
- **Calendar:** records placed by a date field.
- **Form:** record entry using the current schema.

Adjust the layout field pickers, filters, and visible columns, then choose
**Save view**. Lattice stores the view as readable YAML under the package's
`views/` directory.

## Add fields and relationships

Choose **Add column**. In addition to text, numbers, booleans, dates, and other
typed fields, the current data engine supports:

- relations to another table or compatible package;
- junction-backed many-to-many relations;
- formulas;
- lookups through a relation;
- rollups with an aggregate.

Open a record for a detail surface and relation pickers. A stale revision banner
means the package changed elsewhere; reload before continuing rather than
overwriting newer rows.

## Use forms and interfaces

Choose **Forms** to open a package form and submit a new record. Interface
resources can combine metrics, charts, maps, saved views, forms, and resource
links in a resizable grid backed by the same package data.

For larger append-oriented facts, use a [Dataset](/docs/datasets-and-charts/)
instead of stretching a mutable table beyond its intended role.
