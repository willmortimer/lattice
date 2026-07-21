---
title: Core concepts
description: The small set of ideas that explain how Lattice behaves.
---

## Workspace

A workspace is a real directory with a readable `lattice.yaml` manifest.
Canonical resources remain inspectable outside the app.

## Resource

Page, Canvas, Table, Dataset, Notebook, Task, Workflow, Artifact, and File are
distinct resources. Each keeps a format and renderer appropriate to its work.

## View

A view presents another resource. A board, chart, map, form, or interface does
not silently copy or own the underlying data.

## Command and revision

Changes made through Lattice are validated semantic commands. Changes made by
another program are legitimate external revisions. Lattice distinguishes them
rather than inventing history it did not observe.

## Inspect

Inspect is the contextual place for properties, source, links, history, schema,
permissions, logs, and diagnostics. Advanced behavior stays attached to the
resource instead of living in a separate expert application.

## Local-first

Ordinary work commits locally and does not wait for a network. Optional sync is
replication over the workspace, not the canonical format.

Read the complete [principles and invariants](https://github.com/willmortimer/lattice/blob/main/docs/02-principles-and-invariants.md).
