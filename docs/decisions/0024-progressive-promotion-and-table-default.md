# ADR 0024: Progressive promotion is the primary disclosure model

## Status

Accepted.

## Decision

Users begin with a small vocabulary of Page, Canvas, Table, Notebook, and File. Richer implementation categories are revealed only when requirements cross a capability boundary.

`/table` creates a SQLite-backed typed table. Relations, views, forms, actions, interfaces, analytical storage, and sheet behavior are promoted progressively without changing the resource's user-facing identity unnecessarily.

## Consequences

The product avoids create-menu choice paralysis while retaining deep architecture. Promotions must preserve identity, links, history, layout, and compatible content.
