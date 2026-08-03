---
title: HTTP API contract
description: Localhost HTTP API shared with MCP and desktop bridges.
---

# HTTP API contract

Lattice provides a local HTTP API for programmatic access to the open
workspace. MCP and several desktop bridges share the same executor path.

## Status

| Area | Status |
| --- | --- |
| Local authenticated HTTP for the open workspace | [x] Shipped |
| Read / search / context builders | [x] Shipped |
| Proposal endpoints | [x] Shipped |
| Versioned OpenAPI publish on the docs site | [ ] Near |
| Hosted multi-tenant public API | [ ] Near |

## Rules

1. Default assumption: **loopback only**, tied to a running daemon/session.
2. Do not publish the local API port to the public internet.
3. Prefer the same semantic commands as the CLI (`page.*`, `dataset.*`, …).
4. Agents should prefer proposal endpoints for writes.

## Discovery

Until the OpenAPI document is published on the docs site, treat the daemon’s
live route table and the public client docs in `lattice` as the engineering
source. This page is the stable *product* contract summary.

## Related

- [CLI contract](/docs/cli-contract/)
- [MCP contract](/docs/mcp/)
- Open formats: [`/open/`](/open/)
