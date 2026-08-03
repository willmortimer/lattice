---
title: CLI contract
description: Public lattice CLI surface for humans and agents.
---

# CLI contract

The `lattice` CLI talks to the same semantic command core as the desktop app.
Treat this page as the public contract summary. Command help (`lattice --help`,
`lattice <command> --help`) is authoritative for flags on a given build.

## Status

| Area | Status |
| --- | --- |
| Workspace init / info / ls / validate | [x] Shipped |
| Page create / update / search / backlinks | [x] Shipped |
| Table create / import / show / views | [x] Shipped |
| Record insert / update / delete | [x] Shipped |
| History / undo-compatible journal inspection | [x] Shipped |
| Dataset query helpers | [x] Shipped |
| Publish export (page / interface / artifact / deck) | [x] Shipped |
| Daemon attach helpers | [x] Shipped |
| Full workflow authoring from CLI only | [ ] Near |
| Cloud account management from CLI | [ ] Near |

## Everyday examples

```sh
lattice init ~/Work/Research --title "Research" --template research
cd ~/Work/Research
lattice info
lattice ls
lattice validate

lattice page create Notes/Idea.md --content "# Idea"
lattice search "idea"
lattice table create CRM.data --title "CRM" --table contacts
lattice table import --csv contacts.csv --name CRM --table contacts
```

## Rules agents should follow

1. Prefer commands that create **proposals** when mutating shared workspaces.
2. Do not assume network or cloud features are available.
3. Treat the workspace directory as the source of truth; do not invent a shadow
   document store.
4. When unsure about a flag, run `--help` for that build rather than guessing.

## Related

- [MCP contract](/docs/mcp/)
- [HTTP API contract](/docs/api/)
- Open formats pack: [`/open/`](/open/)
