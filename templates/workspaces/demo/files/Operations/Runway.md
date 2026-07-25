---
title: Runway model
export_policy: allow
---


# Runway model

`Operations/Company.data` keeps mutable company facts in SQLite:

- expenses;
- vendors;
- monthly budgets;
- monthly revenue.

Expense intake updates actual spend and the executive operating-result metric.
It does not rewrite revenue. Analytical snapshots may later materialize the
operational records into Parquet without changing the source-of-truth model.

The fixture uses simple synthetic numbers so a recording can explain the
relationship clearly. It is not an accounting system or financial forecast.
