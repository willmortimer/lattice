# Data Applications and the Airtable Model

## Product model

Airtable is best understood as a relational application builder disguised as a friendly spreadsheet. Lattice should adopt its progressive workflow while using open local resources.

```text
Workspace
└── Data application
    ├── SQLite database
    ├── tables and typed fields
    ├── linked records
    ├── views
    ├── forms
    ├── interfaces
    ├── buttons/actions
    └── automations
```

## Package layout

```text
Hiring Pipeline.data/
├── README.md
├── app.yaml
├── database.sqlite
├── schema.sql
├── migrations/
├── views/
├── forms/
├── interfaces/
├── workflows/
└── adapters/
    └── drizzle/
```

SQLite is canonical. Drizzle is an optional generated or maintained adapter for TypeScript development, migrations, and AI-authored custom applications.

## Why SQLite

- Real schema and constraints.
- Transactions.
- Foreign keys.
- Indexes.
- Full-text search.
- JSON support.
- Stable file format.
- Excellent local performance.
- Broad external tooling.
- Direct access from Rust, Python, JavaScript, and CLI tools.

## Progressive workflow

Lattice should let a user evolve gradually:

```text
Paste or import CSV
    ↓
profile fields
    ↓
open as simple table
    ↓
convert to SQLite data app
    ↓
assign semantic types
    ↓
extract repeated values into linked tables
    ↓
create views and forms
    ↓
build interfaces and actions
    ↓
add workflows or custom apps
```

Users should not need to design a normalized schema before starting.

## Typed fields

Native semantic types:

- Text and long text.
- Rich text.
- Integer, decimal, currency, percentage.
- Boolean.
- Date, datetime, duration.
- URL, email, phone.
- Enum and multi-enum.
- Attachment.
- User.
- Relation.
- Lookup.
- Rollup.
- Formula.
- Geolocation.
- JSON.
- Page/resource reference.
- Artifact/app reference.
- Generated/AI field.

Storage remains ordinary SQLite types and tables. Presentation and semantic metadata live in `app.yaml`.

## Linked records

Linked-record UX should make relational modeling approachable:

- Search and select related records.
- Create related record inline.
- Preview related records.
- Show reverse relationships automatically.
- Filter selectable records.
- Choose display fields.
- Traverse relationships in interfaces.
- Generate relationship diagrams.

Underneath, use foreign keys and junction tables.

## Views

Views are saved queries plus presentation:

- Grid.
- List.
- Record detail.
- Kanban.
- Calendar.
- Timeline.
- Gantt.
- Roadmap.
- Gallery.
- Form.
- Map.
- Chart.
- Pivot.
- Dashboard.
- Approval queue.
- Workload.
- Custom artifact or app.

A view never duplicates records.

The view schema (`ViewDef`/`ViewLayout` in `lattice-data`) supports six layout
types: `grid`, `list`, `board`, `gallery`, `calendar`, and `form`. Phase 2
desktop rendering implements `grid` (default), `list`, `board`, and `gallery`;
a view saved with `calendar` or `form` loads and validates correctly, but the
desktop shell currently renders it with the `grid` layout until dedicated UI
lands in a follow-up.

- **Grid** — editable spreadsheet surface (default for saved views and the built-in `All` view).
- **List** — scrollable rows using the first non-`id` column as the title and the next as a subtitle; row click opens record detail.
- **Board** — kanban lanes grouped by `layout.group_by` when set, otherwise a column named `status`, otherwise the first text/boolean column. Row cards reuse the list title/subtitle fields and open record detail on click.
- **Gallery** — card grid using `layout.cover_field` when set, otherwise the first image-like text column (for example `photo` or `cover`), otherwise the primary title text in the cover area. Card click opens record detail.
- **Calendar** — records placed on a calendar using `layout.date_field` as the date column.
- **Form** — single-record input surface using `layout.columns` for field order; no new required field.

Layout fields are exclusive to their layout type and are rejected otherwise:

- `layout.group_by` — board only.
- `layout.cover_field` — gallery only.
- `layout.date_field` — calendar only.

```yaml
format: lattice-view
version: 1
source:
  database: ../database.sqlite
  table: candidates
layout:
  type: board
  group_by: status
filter:
  - field: archived
    operator: equals
    value: false
```

List views omit `group_by`:

```yaml
layout:
  type: list
```

Gallery views set `cover_field` instead of `group_by`:

```yaml
layout:
  type: gallery
  cover_field: photo
```

Calendar views set `date_field`:

```yaml
layout:
  type: calendar
  date_field: due_date
```

Form views reuse `columns` to order fields; no layout-specific field is required:

```yaml
layout:
  type: form
  columns: [name, email, status]
```

## Forms

Forms map input into transactions:

- Create or update records.
- Upload attachments.
- Create related documents.
- Create relationships.
- Trigger approved workflows.
- Validate data.
- Support public or internal publishing.

Form definitions are readable YAML/JSON and can render in Lattice or a published app.

## Interfaces

Interfaces are canvas-based frontends over shared data:

- Record list and selector.
- Detail panel.
- Related-record list.
- Editable fields.
- Metrics and charts.
- Buttons.
- Forms.
- Documents and notebook outputs.
- Conditional visibility.
- Role-specific layouts.

This permits Airtable-style operational apps while retaining normal documents and arbitrary artifacts.

## Formulas, lookups, and rollups

Support two levels:

### Friendly expression layer

```text
{price} * {quantity}
```

### SQL layer

```sql
SELECT SUM(amount_cents)
FROM line_items
WHERE invoice_id = invoices.id
```

Lookups and rollups compile to SQL views, generated columns, cached fields, or runtime queries. Generated SQL remains inspectable.

Do not create a proprietary DAX-like language.

## Actions and buttons

A button invokes a semantic command, task, workflow, query, or proposed transaction.

```yaml
label: Generate company brief
action:
  type: task.run
  task: ../../Scripts/Generate Company Brief.task/task.yaml
input:
  company_id: $record.id
approval: preview-transaction
```

## Generated fields

Generated/AI fields declare:

- Input fields.
- Prompt or classifier.
- Provider profile.
- Refresh mode.
- Invalidating inputs.
- Output type.
- Provenance.
- Human-edit behavior.

Human edits are not overwritten silently.

## Schema inspection

Advanced mode exposes:

- `schema.sql`.
- Migrations.
- Indexes.
- Query plan.
- Constraints.
- Optional Drizzle schema.
- Raw SQLite opening.

## Collaboration and sync

SQLite file copying is not the multiwriter protocol. Lattice records semantic row and schema operations, revisions, and conflicts. The database remains materialized locally.

## What remains outside core

CRM, recruiting, inventory, project management, editorial calendars, and other domains are capability packs or templates built on the generic data-application primitives.
