---
title: Cloud and sync
description: What remains free and local, what the planned paid cloud service adds, and how sync preserves open files.
---

Lattice Cloud is in development and is not required to use the local app.

## Free local app

The intended local product boundary includes:

- local workspaces and open formats;
- import, export, and backups;
- pages, canvas, tables, datasets, notebooks, and local execution;
- local search and user-selected model integrations;
- CLI, local API, MCP, plugins, and self-hosting interfaces.

These capabilities operate on your computer and are not intended to require a
hosted subscription.

## Planned paid managed service

Reasonable hosted services include:

- encrypted personal sync across devices;
- shared workspaces, comments, and presence;
- managed storage, backups, and retention;
- team administration, OIDC, policy, and audit retention;
- managed connectors and server-side automation workers;
- hosted notebook/GPU execution and public publishing.

No plans or prices have been announced.

## How sync is designed to work

A change commits locally first, materializes into the canonical resource, and
enters a local outbox. Background sync then transmits an idempotent operation or
snapshot and records acknowledgement. Losing network access must not block
ordinary editing.

Replication metadata is not the file format. Markdown, SQLite, Parquet, JSON
Canvas, ipynb, and other resources remain the materialized workspace, while
sync logs and CRDT data remain replaceable machinery.

Read the deeper [sync architecture](https://github.com/willmortimer/lattice/blob/main/docs/22-sync-cloud-backend-history-collaboration.md).
