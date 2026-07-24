# Automation, Events, Workflows, and Daemon

## Why automation belongs in the platform

Leaving scheduling and hooks entirely to shell scripts would recreate plugin chaos. Lattice should provide a small generic automation kernel while keeping domain workflows external and inspectable.

## Execution modes

- Manual command.
- Event-triggered workflow.
- Scheduled job.
- Derived-resource build.
- External webhook or connector event.
- On-next-open fallback.
- Remote worker execution.

## Event model

Core events:

```text
workspace.opened
workspace.synced
resource.created
resource.changed
resource.deleted
external-file.changed
page.tagged
dataset.record-inserted
dataset.record-updated
dataset.schema-changed
form.submitted
artifact.event
notebook.executed
query.completed
telemetry.received
schedule.fired
```

Events are typed, versioned, and do not expose private UI implementation details.

## Hook categories

### Validators

Run before commit and may accept, reject, or warn. They must be bounded and should not perform unrelated writes.

### Transaction transforms

Add related operations to a proposed transaction, such as creating a page when a record is inserted.

### Post-commit subscribers

Run after canonical commit. Failures do not roll back the user's saved edit.

### Scheduled jobs

Durable independent execution.

### File watchers

React to external changes after stable-write detection.

## Workflow format

Bounded v1 runtime (`lattice-commands` + desktop) supports:

```yaml
format: lattice-workflow
version: 1
name: Simple proposal workflow
enabled: true
trigger:
  type: manual
  # type: resource.changed
  # paths: [Notes/**, Data/*.csv]
  # type: form.submitted
  # package: Data/CRM.data
  # form: ContactIntake
  # type: schedule
  # interval_seconds: 3600
  # cron: "0 2 * * *"
  # timezone: America/Los_Angeles
steps:
  - id: run-hello
    action: task.run
    with:
      task: Hello.task
  - id: create-proposal
    action: proposal.create
    with:
      summary: Create a reviewable page
      commands:
        - type: page-create
          path: Notes/FromWorkflow.md
          content: "# From workflow\n"
  - id: notify
    action: notification
    with:
      message: Done
```

v1 triggers: `manual`, `resource.changed` (path globs; debounced in the desktop
watcher), `form.submitted` (form path or package + form id; wired from
`insert_record` when a package form submits), and `schedule`
(`interval_seconds` and/or `cron`, optional `timezone`). `enabled: false` skips
automatic triggers; manual Run still executes.

v1 steps: `task.run` (delegates to TaskRunner), `proposal.create` (source type
`workflow`), optional `notification` (log only). Leaf steps may set optional
`retry` (`max_attempts` ≥ 1, `backoff_seconds` interruptible sleep between
failures — cancel wakes the wait). A step with a non-empty `parallel` child list
(action `parallel` or omitted) runs those children concurrently (bounded, then
join) before the next top-level step. Unknown actions/triggers are rejected at
parse time. Run history is stored under `.lattice/workflows/runs/` (step results
may include `attempts` when > 1). Execution results carry `proposalIds` (all ids
from the run; `proposalId` remains the first for compatibility).

**Retry safety:** `proposal.create` is idempotent per run via key
`{execution_id}:{step_id}` stamped on `ProposalSource` — a retry that already
persisted a pending proposal returns that proposal instead of minting a
duplicate. `task.run` retries require either `execution.idempotent: true` on the
task manifest or an explicit `with.allow_unsafe_retry: true` on the step; without
one of those, `max_attempts > 1` is rejected before the first attempt.

Example task declaration:

```yaml
# task.yaml
execution:
  idempotent: true
```

Unsafe override on a workflow step:

```yaml
- id: side-effect
  action: task.run
  retry:
    max_attempts: 3
    backoff_seconds: 2
  with:
    task: SideEffect.task
    allow_unsafe_retry: true
```

**Schedule firing (latticed):** when a workspace session is open, the daemon
polls enabled `schedule` workflows and fires those with a due `interval_seconds`
(trigger label `schedule`). Cron-only schedules are parsed/validated but not
evaluated yet (set `interval_seconds` to fire; cron evaluator is TODO). Durable
job queues, a known-workspace registry, and closed-desktop cron remain out of
scope — interval schedules do **not** claim durability while the desktop is
closed.

**Job status (tray / HTTP):** daemon-owned schedule runs register in an
in-memory job registry under a stable `executionId` (same id in
`.lattice/workflows/runs/*.json`). Localhost routes:

| Method | Path | Purpose |
| --- | --- | --- |
| `POST` | `/v1/jobs/list_active` | In-flight daemon jobs |
| `POST` | `/v1/jobs/list_recent` | Recent + optional workspace disk history |
| `POST` | `/v1/jobs/get` | One job by `executionId` |
| `POST` | `/v1/jobs/cancel` | Cooperative cancel (daemon-owned only) |

On OpenWorkspace / schedule tick, stranded on-disk `running` records are marked
`abandoned` (process exited before finish). Desktop tray merges in-process
`WorkflowState` runs with daemon jobs (HTTP when `LATTICE_AUTH_TOKEN` + API port
are available, else disk `running` schedule records). Cancel ownership is
explicit: `desktop` vs `daemon`.

Example with retry and parallel fan-out:

```yaml
steps:
  - id: flaky-task
    action: task.run
    retry:
      max_attempts: 3
      backoff_seconds: 2
    with:
      task: Flaky.task
  - id: fan
    action: parallel
    parallel:
      - id: left
        action: notification
        with:
          message: left
      - id: right
        action: notification
        with:
          message: right
  - id: after
    action: notification
    with:
      message: joined
```

Earlier illustrative format (broader than v1):

```yaml
format: lattice-workflow
version: 1
name: Create company research page
trigger:
  type: dataset.record-created
  dataset: ../Data/CRM.data/database.sqlite
  table: companies
conditions:
  - expression: record.research_page_id == null
steps:
  - id: create-page
    action: page.create-from-template
    with:
      template: ../Templates/company-research.md
  - id: link-page
    action: dataset.update-record
    with:
      page_id: $steps.create-page.resource_id
```

## BPMN and DMN

Support BPMN as an optional open visual workflow model and DMN for decision tables. Lattice YAML remains the simple native automation format. Adapters map supported BPMN/DMN constructs to the execution kernel.

## Scheduler

Support:

- One-time jobs.
- Intervals.
- Cron.
- Calendar-aware recurrence.
- Named time zones.
- Missed-run policy.
- Run-on-next-open.
- Local daemon, server, or remote worker target.

## Local daemon

`latticed` handles:

- Long-lived schedules.
- File watching while UI is closed.
- Local API and MCP.
- Connector refreshes.
- Data extracts.
- Artifact/app builds.
- Jupyter kernel and job supervision.
- OTLP ingestion.
- Sync.

The daemon is optional for ordinary editing.

## Task runtimes

- Python with `uv`.
- Jupyter notebook or kernel.
- Node/TypeScript.
- Native executable.
- Shell/PowerShell.
- Nix environment.
- Container.
- WASI component.
- Remote runner.

Each task declares inputs, outputs, capabilities, environment, limits, and execution target.

## Derived resources

A derived resource declares inputs and builder
([ADR 0022](decisions/0022-derived-resources-have-lineage.md)):

```yaml
format: lattice-derived-resource
version: 1
output: ./dist/index.html
inputs:
  - ../../Data/Companies.data/database.sqlite
  - ./queries/summary.sql
  - ./src/**
builder:
  task: ./Build Dashboard.task/task.yaml
refresh:
  mode: on-demand
```

Naming: `*.derived.yaml` (or `.yml`) is classified as
`ResourceKind::Derived`. Relative paths resolve from the manifest directory.

Lattice tracks `current` / `stale` / `building` / `failed` by hashing listed
input files (v1 also expands simple `*` / `**` globs), the builder task
package, and the declared output, then comparing against lineage recorded
under `.lattice/derived/`. `current` requires all of those to exist with
matching hashes; otherwise Lattice reports structured `staleReasons`
(`never-built`, `input-changed`, `input-missing`, `output-missing`,
`output-changed`, `builder-failed`, `builder-changed`).

Rebuild runs the declared `builder.task` through the existing task runner with
`LATTICE_DERIVED_OUTPUT` / `LATTICE_DERIVED_STAGING` pointing at
`.lattice/derived/staging/<build-id>/`. On success Lattice verifies the staged
artifact and atomically promotes it to the declared output path; on failure or
interruption the previous output is left untouched (last-known-good). Per-resource
build locks serialize concurrent rebuilds; abandoned staging directories are
cleaned up.

Declared inputs, builder task, and output also surface as `input` /
`output` edges in the Inspect relationship graph (see
[resource runtime contracts](./39-resource-runtime-contracts.md#relationship--lineage-graph-inspect)).
Workflow trigger and step resource refs surface as `workflow` edges in the
same panel.

## Failure handling

- Durable job record.
- Structured logs and trace.
- Retry policy.
- Dead-letter/failed queue.
- Cancellation.
- Timeout.
- Last-known-good output.
- No silent failure.

## Approval policy

Workflows may require:

- Every-run approval.
- First-run approval.
- Proposed transaction review.
- Auto-approval under path/row-count limits.
- Trusted signed pack policy.

## Visual workflow builder

The visual builder edits the same YAML or BPMN resource. It is not a separate opaque workflow database.
