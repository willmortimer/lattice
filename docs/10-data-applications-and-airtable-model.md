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

### MVP cell shape (Phase 1)

Relation fields are typed in `app.yaml` and stored as JSON TEXT in SQLite—no junction tables in this MVP:

```yaml
# app.yaml (excerpt)
tables:
  contacts:
    columns:
      company:
        type: relation
        relation_table: companies
```

```json
// CellValue over IPC / command payloads (externally tagged)
{ "Relation": { "record_ids": ["0195f0a2-…", "0195f0a3-…"] } }
```

```text
// SQLite TEXT encoding for the same cell
["0195f0a2-…","0195f0a3-…"]
```

- `relation_table` names a target table in the same `.data` package. Cross-table
  relations within one package are supported (for example `contacts.company` →
  `companies` in First Look `CRM.data`). Cross-package links, junction tables,
  Lookup, and Rollup remain later work.
- Cells hold zero or more linked record ids; insert/update validates each id
  exists in the target table.
- On `RecordDelete` / `delete_row`, Lattice strips the deleted id from every
  relation column in the package whose `relation_table` points at the deleted
  row's table (self-relations and cross-table inbound links), in the same SQLite
  transaction as the DELETE. Command undo restores the deleted row **and** the
  prior inbound relation cells captured in history (`DeletedRowSnapshot` /
  `RelationStrip`).

### Relation labels and `relation_targets`

`open_data_app` includes a `relation_targets` map: for each distinct
`relation_table` referenced by the active table's columns, the snapshot carries
target rows (id + values) used to resolve display labels. The shell builds a
label index from name-like fields (`name`, `title`, `label`) and falls back to
the first text value or raw id.

- **Grid** — relation cells use the label index when present.
- **List, board, gallery, calendar** — title, subtitle, cover, and date fields
  resolve relation columns through the same index (not raw id strings).
- **After mutate** — when the active table is itself a relation target for
  other tables, insert/update/delete on that table patch `relation_targets` in
  the shell snapshot so pickers and labels stay current without a full reopen.
  Rows deleted through `RecordDelete` are removed from the index when the backend
  strips inbound links.

### Record detail

Record detail is the editable surface for a single row:

- **Outbound relations** — relation-typed columns use a searchable picker backed
  by `relation_targets` for the column's `relation_table`.
- **Inbound / reverse links (read-only)** — a **Linked from** section lists rows
  whose relation cells point at the open record. Self-relations are discovered
  from the active table's rows; cross-table inbound links (for example contacts
  pointing at a company) use `relation_targets`. Each entry shows the source
  row label and the linking column (and source table when different). Sources on
  the active table are navigable; cross-table sources are display-only in v1.

### Template seed resolution

When a workspace template provisions `.data` package rows, relation cell values
in template JSON may list **record ids** or the target row's **`name`** text.
The provisioner resolves each reference against the target table before insert;
unresolved references fail template validation. This keeps hand-authored seeds
readable (for example `"company": ["Analytical Engines"]`) while storing canonical
ids in SQLite.

Lookup, Rollup, junction tables, and cross-package relation UX remain later
work.

Linked-record UX should make relational modeling approachable:

- Search and select related records.
- Create related record inline.
- Preview related records.
- Show reverse relationships automatically.
- Filter selectable records.
- Choose display fields.
- Traverse relationships in interfaces.
- Generate relationship diagrams.

Underneath, use foreign keys and junction tables for richer models over time; the MVP stores multi-record links as JSON TEXT as above.

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
desktop rendering implements `grid` (default), `list`, `board`, `gallery`,
`calendar`, and `form`.

- **Grid** — editable spreadsheet surface (default for saved views and the built-in `All` view).
- **List** — scrollable rows using the first non-`id` column as the title and the next as a subtitle; row click opens record detail.
- **Board** — kanban lanes grouped by `layout.group_by` when set, otherwise a column named `status`, otherwise the first text/boolean column. Row cards reuse the list title/subtitle fields and open record detail on click.
- **Gallery** — card grid using `layout.cover_field` when set, otherwise the first image-like text column (for example `photo` or `cover`), otherwise the primary title text in the cover area. Card click opens record detail.
- **Calendar** — month (and optional week) calendar using `layout.date_field` when set, otherwise the first `date` column, otherwise a date-like column name (for example `due_date`). Records parse `YYYY-MM-DD` and ISO datetimes to a day; unparseable values appear in an **Undated** bucket. Prev/next navigation, today shortcut, and event click open record detail.
- **Form** — create-focused field form using `layout.columns` for field order when set, otherwise all non-`id` columns. Submit inserts a row through the same `insert_record` command path as the grid. After create, the form clears and offers **Open record** for edit in record detail; a compact recent-records list links to existing rows. Public publish and workflow triggers are out of scope.

Desktop **Save view** persists the selected layout type and layout-specific fields
(`group_by`, `cover_field`, `date_field`) through `save_data_view`; reloading the
view restores the same layout. Hand-authored YAML remains supported.

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
