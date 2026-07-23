---
title: Agent first look
---

# Agent first look

Rehearsable **inspect → propose → approve** path for agents and tasks on the
First Look workspace. Uses AG1 MCP helpers (`get_dataset_schema`,
`profile_dataset`, `propose_interface`) or the injected `lattice` SDK inside
tasks. Proposals land in the inbox — nothing applies until you approve.

## Task path (native desktop)

1. Open `Tasks/AgentFirstLook.task` → **Run** (needs `uv` + injected `lattice`).
2. The task inspects `Data/Orders.dataset` and `Data/Events.dataset` (CSV
   headers + Parquet partition counts), prints a JSON summary, then calls
   `lattice.propose_interface` for `CRM.data/interfaces/AgentDigest.interface.yaml`.
3. Open the **Proposals** inbox → approve the resource-create proposal.
4. Open `CRM.data` → **Interfaces** → **Agent digest** — two metric tiles over
   Events signups and Orders revenue.

Same inbox semantics as [[Proposals/README]] and the Contact intake workflow.

## MCP path (daemon)

With `latticed` serving the open workspace, an agent can:

1. `get_dataset_schema` on `Data/Orders.dataset` and `Data/Events.dataset`.
2. `profile_dataset` on the same paths (bounded DuckDB `SUMMARIZE`).
3. `propose_interface` (or `propose_workflow`) with validated YAML — no apply.

Sample JSON-RPC transcript: `docs/dev/first-look-agent-mcp.md` in the repo.

## Related seeds

- `Tasks/ProposePage.task` — SDK `propose_page` only.
- `Automations/Contact intake.workflow.yaml` — form → workflow → `proposal.create`.
- `CRM.data` → **Interfaces** → **Ops dashboard** — hand-authored multi-component
  interface the agent seed mirrors.
