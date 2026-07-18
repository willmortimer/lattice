---
title: Data Lab
---

# Data Lab

- Register connections in [[Connections/]] — databases, APIs, and file drops.
- Keep canonical datasets in [[Data/]] — see `contacts.csv`.
- Save reusable SQL in [[Queries/]] — start with `example.sql`.
- Put reproducible computation in [[Notebooks/]] — open the example notebook stub.
- Build summaries in [[Dashboards/]].
- Publish narrative findings from [[Reports/]].

## Seeded examples

| Resource | Location |
| -------- | -------- |
| Contact list | [[Data/contacts.csv]] |
| Sample query | [[Queries/example.sql]] |
| Metrics table | `Data/metrics.data` (seeded SQLite package) |
| Starter notebook | [[Notebooks/example.ipynb]] |

## Metrics views

Open `Data/metrics.data` and switch layouts from the view picker. The template
compiler seeds saved views under `Data/metrics.data/views/`:

| View | Layout | Key field |
| ---- | ------ | --------- |
| Board | `board` | `category` |
| Calendar | `calendar` | `recorded_on` |
| Gallery | `gallery` | `metric` (cover) |
| Form | `form` | — |

Board groups metrics by category; calendar plots readings on `recorded_on`.
Gallery uses the metric name as a cover label until you add image columns.
