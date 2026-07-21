---
title: Notebooks, tasks, and workflows
description: Run Jupyter notebooks, package repeatable local tasks, execute bounded workflows, and review proposed changes.
---

Lattice separates interactive exploration, repeatable execution, and canonical
workspace mutation.

## Run a notebook

Open an `.ipynb` file. Markdown and existing outputs render immediately.

- Choose **Run** on one code cell or **Run all** in the notebook toolbar.
- Choose **Cancel** to interrupt the active run.
- The native desktop prefers an out-of-process `ipykernel` and falls back
  visibly when that environment is unavailable.
- Browser-oriented environments may use Pyodide with a bounded mounted-file
  bridge.

Successful outputs are merged back into the Jupyter document with a revision
guard. The notebook remains readable if no kernel can start.

## Run a task

A `*.task/` package declares its runtime, project, command, inputs, outputs, and
timeout in `task.yaml`. Open the package to inspect the manifest, then choose
**Run**. The task surface streams stdout and stderr, shows status and duration,
and links declared outputs. **Cancel** terminates an active task.

Tasks use local providers such as `uv`, Nix, or a configured system runtime.
They should write declared outputs or return a proposal rather than modifying
arbitrary workspace files invisibly.

## Run a workflow

A `*.workflow.yaml` resource coordinates bounded task and proposal steps.
The workflow surface shows whether automatic triggers are enabled, the raw YAML,
step logs, recent executions, and any proposal produced by the run. Manual Run
remains available when triggers are disabled.

## Review a proposal

Open the proposal inbox from the activity rail. A proposal names its source,
summary, affected paths, warnings, and semantic commands. Select the commands
you want and apply them as one transaction, or reject the proposal without
changing canonical files.

This review boundary is also how future agents and hosted automation can remain
useful without receiving ambient write authority.
