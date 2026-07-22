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
(`interval_seconds` and/or `cron`, optional `timezone`; parse/validate only —
no firing loop yet). `enabled: false` skips automatic triggers; manual Run still
executes.

v1 steps: `task.run` (delegates to TaskRunner), `proposal.create` (source type
`workflow`), optional `notification` (log only). Unknown actions/triggers are
rejected at parse time. Run history is stored under `.lattice/workflows/runs/`.
Schedule firing, durable daemon jobs, and a visual editor remain out of scope.

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
input files (v1 also expands simple `*` / `**` globs) and comparing against
lineage recorded under `.lattice/derived/`. Rebuild runs the declared
`builder.task` through the existing task runner and refreshes lineage on
success.

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
