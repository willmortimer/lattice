---
title: Interview Synthesis
tags: [research]
---

# Interview Synthesis

Themes from five fictional discovery calls — seeds for [[Product/Vision]] and CRM
status values in `CRM.data`.

## Recurring requests

1. **Keep my files** — git-friendly Markdown and JSON, not opaque databases.
2. **Typed tables inline** — contacts and tasks beside narrative docs.
3. **Spatial overview** — canvas for strategy, not just pretty wallpapers.
4. **Fast search** — path + body, tolerating long pages like [[Research/Long Read]].

## Representative quotes

| Speaker | Quote | Implied column |
| --- | --- | --- |
| PM | "Board view by status is how I run standup." | `status` |
| Engineer | "Due dates on leads, not on archived contacts." | `due_date` |
| Designer | "Gallery cover from company name is enough for now." | `company` |

## Follow-ups

- [ ] Add saved board view under `CRM.data/views/Board.yaml` (see [[Home#CRM views]])
- [ ] Link interview pages from canvas nodes
- [ ] Export subset to `Data/sample.csv` for comparison

#research
