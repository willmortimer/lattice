---
title: Hackathon demo script
export_policy: allow
---


# Hackathon demo script

## Opening — the workspace operates the company

Open [[Home]] and say:

> Lattice is an open local-first workspace where documents, operational data,
> analytical data, notebooks, canvases, and automation remain inspectable
> resources instead of disappearing into a hosted application.

## Scene 1 — product and engineering

1. Open `Product/Roadmap.data → Product pulse`.
2. Show roadmap status, feature maturity, feedback and decisions.
3. Open `Engineering/Delivery.data → Release room`.
4. Open `Engineering/Build Status.dataset`.
5. Move through Preview, Chart, Profile and Plan.
6. Open [[Engineering/Repository]] and, when authenticated, the connected root.

## Scene 2 — company operations

1. Open `Operations/Company.data → Runway dashboard`.
2. Point out revenue, spend and operating-result metrics.
3. Submit **Expense intake**.
4. Show the expense record appear in the Board or Calendar.
5. Explain that expenses change spend and operating result, not revenue.

## Scene 3 — governed customer feedback

1. Open `CRM/Feedback.data → Feedback operations`.
2. Submit **Feedback intake**.
3. Open the workflow run.
4. Review the proposal.
5. Approve `Proposals/Feedback triage.md`.

## Scene 4 — composition

Open `Hackathon/Pitch.canvas`. Press **Present** (or `P`) to rehearse ordered
camera scenes from `Pitch.canvas.presentation.json`. Advance with arrow keys;
`Esc` exits.

When developing from the private ecosystem checkout, `desktop-dev` can merge a
local `Hackathon/Pitch.deck` overlay for a presenter-native close — that package
is not part of the public First Look template.

## Fallback language

- Local analytics: “DuckDB queries partitioned Parquet and transfers a bounded
  Arrow IPC result into specialized viewers.”
- Connected repository unavailable: “The template keeps network authority
  explicit; this local release evidence is always available.”
- Agent unavailable: run `Tasks/AgentFirstLook.task` for the deterministic
  inspect → propose → approve path.
