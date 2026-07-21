---
title: CLI guide
description: Create, inspect, search, query, and mutate Lattice workspaces from the terminal.
---

The `lattice` CLI uses the same core formats and semantic command model as the
desktop app.

## Create and inspect a workspace

```sh
lattice init ~/Work/Research --title "Research" --template research
cd ~/Work/Research
lattice info
lattice ls
lattice validate
```

Run `lattice templates list` to see current template IDs.

## Work with pages

```sh
lattice page create Notes/Idea.md --content "# Idea"
printf '# Revised idea\n' | lattice page update Notes/Idea.md --stdin
lattice search "revised idea"
lattice backlinks Notes/Idea.md
```

## Work with tables and records

```sh
lattice table create CRM.data --title "CRM" --table contacts
lattice table import --csv contacts.csv --name CRM --table contacts
lattice table show CRM.data
lattice table view list CRM.data
```

Use `lattice record --help` for insert, update, and delete operations. Mutations
are journaled so compatible operations appear in `lattice history` and can be
undone.

## Work with analytical data

```sh
lattice dataset create Data/Events.dataset --title "Events"
lattice dataset import-csv Data/Events.dataset --csv events.csv
lattice dataset show Data/Events.dataset
lattice query --engine duckdb --sql "select count(*) from read_parquet('Data/Events.dataset/facts/**/*.parquet')"
```

## Maintain the workspace

```sh
lattice index
lattice history --limit 30
lattice undo
lattice redo
lattice recover --help
```

Add `--json` where supported when another program should consume the result.
Use `lattice <command> --help` for exact arguments in your installed build.
